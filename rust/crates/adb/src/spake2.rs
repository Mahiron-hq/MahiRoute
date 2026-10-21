// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Safe wrapper around `BoringSSL`'s SPAKE2, which the `boring` crate does not expose.
//!
//! `adbd` authenticates the pairing code with exactly this implementation, so reusing it keeps
//! the wire format identical.

#![allow(unsafe_code)] // FFI into BoringSSL; every call below states why it is sound.

use std::ptr::NonNull;

use boring_sys as ffi;

use crate::error::AdbError;

/// Which side of the exchange this context plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The side that started the pairing (the phone app).
    Alice,
    /// The side that answers (`adbd`).
    Bob,
}

/// One SPAKE2 exchange: produce a message, then combine it with the peer's.
pub struct Spake2 {
    context: NonNull<ffi::SPAKE2_CTX>,
}

// SAFETY: the context is owned exclusively by this value and BoringSSL does not tie it to a thread.
unsafe impl Send for Spake2 {}

impl Spake2 {
    /// Creates a context for `role`; the names bind the exchange to the two parties.
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Pairing`] when `BoringSSL` cannot allocate the context.
    pub fn new(role: Role, my_name: &[u8], their_name: &[u8]) -> Result<Self, AdbError> {
        let role = match role {
            Role::Alice => ffi::spake2_role_t::spake2_role_alice,
            Role::Bob => ffi::spake2_role_t::spake2_role_bob,
        };
        // SAFETY: both pointers and lengths describe live slices for the duration of the call, and
        // BoringSSL copies the names before returning.
        let raw = unsafe {
            ffi::SPAKE2_CTX_new(
                role,
                my_name.as_ptr(),
                my_name.len(),
                their_name.as_ptr(),
                their_name.len(),
            )
        };
        NonNull::new(raw)
            .map(|context| Self { context })
            .ok_or_else(|| AdbError::Pairing("cannot create the SPAKE2 context".to_owned()))
    }

    /// Produces this side's message for `password`; call it once.
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Pairing`] when `BoringSSL` rejects the call, for example when it was
    /// already made.
    pub fn generate_message(&mut self, password: &[u8]) -> Result<Vec<u8>, AdbError> {
        let mut out = [0u8; ffi::SPAKE2_MAX_MSG_SIZE as usize];
        let mut written = 0usize;
        // SAFETY: `context` is a live context; `out` has `max_out_len` writable bytes, `written`
        // is a valid out-pointer and `password` is a live slice.
        let status = unsafe {
            ffi::SPAKE2_generate_msg(
                self.context.as_ptr(),
                out.as_mut_ptr(),
                &raw mut written,
                out.len(),
                password.as_ptr(),
                password.len(),
            )
        };
        if status != 1 || written > out.len() {
            return Err(AdbError::Pairing(
                "SPAKE2 could not generate a message".to_owned(),
            ));
        }
        Ok(out[..written].to_vec())
    }

    /// Combines the peer's message into the shared key.
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Pairing`] when the peer's message is malformed.
    pub fn process_message(&mut self, their_message: &[u8]) -> Result<Vec<u8>, AdbError> {
        let mut key = [0u8; ffi::SPAKE2_MAX_KEY_SIZE as usize];
        let mut written = 0usize;
        // SAFETY: `context` is a live context; `key` has `max_out_key_len` writable bytes,
        // `written` is a valid out-pointer and `their_message` is a live slice.
        let status = unsafe {
            ffi::SPAKE2_process_msg(
                self.context.as_ptr(),
                key.as_mut_ptr(),
                &raw mut written,
                key.len(),
                their_message.as_ptr(),
                their_message.len(),
            )
        };
        if status != 1 || written > key.len() {
            return Err(AdbError::Pairing(
                "the peer's SPAKE2 message was rejected".to_owned(),
            ));
        }
        Ok(key[..written].to_vec())
    }
}

impl Drop for Spake2 {
    fn drop(&mut self) {
        // SAFETY: `context` came from `SPAKE2_CTX_new`, is freed exactly once here, and is not
        // used afterwards.
        unsafe { ffi::SPAKE2_CTX_free(self.context.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: &[u8] = b"adb pair client\0";
    const BOB: &[u8] = b"adb pair server\0";

    fn exchange(alice_password: &[u8], bob_password: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let mut alice = Spake2::new(Role::Alice, ALICE, BOB).expect("alice");
        let mut bob = Spake2::new(Role::Bob, BOB, ALICE).expect("bob");
        let alice_message = alice
            .generate_message(alice_password)
            .expect("alice message");
        let bob_message = bob.generate_message(bob_password).expect("bob message");
        (
            alice.process_message(&bob_message).expect("alice key"),
            bob.process_message(&alice_message).expect("bob key"),
        )
    }

    #[test]
    fn equal_passwords_yield_equal_keys() {
        let (alice, bob) = exchange(b"123456", b"123456");
        assert_eq!(alice, bob);
        assert_eq!(alice.len(), 64);
    }

    #[test]
    fn different_passwords_yield_different_keys() {
        let (alice, bob) = exchange(b"123456", b"123457");
        assert_ne!(alice, bob);
    }

    #[test]
    fn messages_are_32_bytes_and_fresh() {
        let mut first = Spake2::new(Role::Alice, ALICE, BOB).expect("first");
        let mut second = Spake2::new(Role::Alice, ALICE, BOB).expect("second");
        let a = first.generate_message(b"123456").expect("message");
        let b = second.generate_message(b"123456").expect("message");
        assert_eq!(a.len(), 32);
        assert_ne!(a, b, "each exchange must use fresh randomness");
    }

    #[test]
    fn a_context_cannot_generate_twice() {
        let mut alice = Spake2::new(Role::Alice, ALICE, BOB).expect("alice");
        alice.generate_message(b"123456").expect("first");
        assert!(alice.generate_message(b"123456").is_err());
    }

    #[test]
    fn processing_before_generating_is_rejected() {
        let mut alice = Spake2::new(Role::Alice, ALICE, BOB).expect("alice");
        assert!(alice.process_message(&[0; 32]).is_err());
    }
}
