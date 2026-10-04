# ─── Tally release R8 rules ─────────────────────────────────────────────────
# Tally holds no INTERNET permission and reflects over nothing of its own. Room, Hilt, WorkManager,
# Compose and kotlinx-serialization ship consumer rules for their generated code; these cover the
# seams they do not.

# Readable crash reports without original file names.
-keepattributes SourceFile,LineNumberTable
-renamesourcefileattribute SourceFile

# ── Enums (PERSISTENCE-CRITICAL) ────────────────────────────────────────────
# Room stores TxType, AccountType, CategoryKind and Frequency by NAME, and the JSON backup writes
# the same names. Were R8 to rename the constants, every stored row and every backup would fail to
# parse after an update. Keep enum members so the strings keep round-tripping.
-keepclassmembers enum * {
    <fields>;
    public static **[] values();
    public static ** valueOf(java.lang.String);
}

# ── Backup DTOs ─────────────────────────────────────────────────────────────
# The backup file format is a public contract with the owner's own files: keep the serializable
# classes and their generated serializers whole.
-keep class com.tally.core.*Dto { *; }
-keep class com.tally.core.BackupFile { *; }
-keepclassmembers class com.tally.core.** {
    *** Companion;
    kotlinx.serialization.KSerializer serializer(...);
}

# ── Room entities and projections ───────────────────────────────────────────
-keep class com.tally.app.data.db.** { *; }

# ── WorkManager + Hilt workers ──────────────────────────────────────────────
-keep class * extends androidx.work.ListenableWorker { *; }
-keep @androidx.hilt.work.HiltWorker class * { <init>(...); }
-keep @dagger.assisted.AssistedFactory class * { *; }

-dontwarn kotlinx.coroutines.**
