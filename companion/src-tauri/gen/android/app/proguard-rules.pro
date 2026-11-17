# ProGuard rules for Texure Companion Android app

# Keep Shizuku remote process reflection method and class members
-keepclassmembers class rikka.shizuku.Shizuku {
    private static rikka.shizuku.ShizukuRemoteProcess newProcess(java.lang.String[], java.lang.String[], java.lang.String);
}
-keep class rikka.shizuku.ShizukuRemoteProcess { *; }

# Keep companion plugin and service classes
-keep class com.texure.companion.ShizukuPlugin { *; }
-keep class com.texure.companion.CompanionDaemonService { *; }
