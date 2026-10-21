// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

// Release signing comes from Gradle properties (-P or ~/.gradle/gradle.properties); without all four
// the release build is signed with the debug key so that it still installs for testing.
val releaseSigning = listOf("RELEASE_STORE_FILE", "RELEASE_STORE_PASSWORD", "RELEASE_KEY_ALIAS", "RELEASE_KEY_PASSWORD")
    .associateWith { providers.gradleProperty(it).orNull }

android {
    namespace = "io.github.mahiron_hq.mahiroute"
    compileSdk = 36

    defaultConfig {
        applicationId = "io.github.mahiron_hq.mahiroute"
        minSdk = 31
        targetSdk = 36
        versionCode = 1
        versionName = "1.0.0"
        ndk { abiFilters += "arm64-v8a" }
    }

    signingConfigs {
        if (releaseSigning.values.all { it != null }) {
            create("release") {
                storeFile = file(releaseSigning.getValue("RELEASE_STORE_FILE")!!)
                storePassword = releaseSigning.getValue("RELEASE_STORE_PASSWORD")
                keyAlias = releaseSigning.getValue("RELEASE_KEY_ALIAS")
                keyPassword = releaseSigning.getValue("RELEASE_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    lint {
        abortOnError = true
        warningsAsErrors = false
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
        allWarningsAsErrors.set(true)
    }
}

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.androidx.core.ktx)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.graphics)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons.core)
    testImplementation(libs.junit)
    testImplementation(libs.org.json)
}

val rustWorkspace = rootProject.file("rust")
val jniLibsDir = layout.projectDirectory.dir("src/main/jniLibs")

val isWindows = System.getProperty("os.name").startsWith("Windows")
val ndkHome: String? = System.getenv("ANDROID_NDK_HOME") ?: System.getenv("ANDROID_NDK_ROOT")

val buildRustLibrary by tasks.registering(Exec::class) {
    group = "build"
    description = "Cross-compiles the Rust engine with cargo-ndk and bundles the C++ runtime it links."
    workingDir = rustWorkspace
    commandLine(
        "cargo", "ndk", "--platform", "31", "-t", "arm64-v8a",
        "-o", jniLibsDir.asFile.absolutePath, "build", "--release", "-p", "mahiroute-jni",
    )
    if (isWindows && ndkHome != null) {
        // BoringSSL is built by CMake; without Ninja it needs the NDK's make and a POSIX shell.
        environment("CMAKE_GENERATOR", "Unix Makefiles")
        val extraPath = listOf("$ndkHome/prebuilt/windows-x86_64/bin", "C:/Program Files/Git/usr/bin")
        environment("PATH", extraPath.joinToString(File.pathSeparator) + File.pathSeparator + System.getenv("PATH"))
    }
    inputs.files(fileTree(rustWorkspace) { include("Cargo.toml", "Cargo.lock", "crates/**/Cargo.toml", "crates/**/src/**") })
    outputs.dir(jniLibsDir)
    doLast {
        val runtime = fileTree("$ndkHome/toolchains/llvm/prebuilt") { include("**/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so") }.files.first()
        runtime.copyTo(jniLibsDir.file("arm64-v8a/libc++_shared.so").asFile, overwrite = true)
    }
}

tasks.named("preBuild") { dependsOn(buildRustLibrary) }
