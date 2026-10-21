# The shell-side helper is started by class name through app_process.
-keep class io.github.mahiron_hq.mahiroute.helper.** { *; }

# Rust exports the native methods by name and calls the listener by reflection-free JNI lookup.
-keepclasseswithmembernames class * { native <methods>; }
-keep class io.github.mahiron_hq.mahiroute.core.Native { *; }
-keep interface io.github.mahiron_hq.mahiroute.core.StateListener { *; }
-keepclassmembers class * implements io.github.mahiron_hq.mahiroute.core.StateListener { *; }
