# ─── Forge release R8 / ProGuard rules ───────────────────────────────────────
# Forge is fully offline (no INTERNET permission), and its JSON/CSV exports are built
# by hand rather than reflected over. It is NOT free of a serialization library: :app
# api-depends on :shared, which api-depends on kotlinx.serialization for the wear
# protocol, so those @Serializable classes go through this R8 pass — they survive on
# the rules embedded in the kotlinx-serialization artifacts, not on anything here. So
# almost everything R8 needs already comes from the "consumer" rules bundled in the
# AndroidX / Hilt / Room / Health-Connect / Glance AARs. The rules below cover the few
# seams those don't, with the persistence-critical enum-name preservation first.
# The :wear module has its own copy of the enum rule — same protocol, two R8 runs.

# Readable crash reports (line numbers) without leaking original file names.
-keepattributes SourceFile,LineNumberTable
-renamesourcefileattribute SourceFile

# ── Enums (PERSISTENCE-CRITICAL) ──────────────────────────────────────────────
# A large amount of Forge state is persisted by enum *name*: Room TypeConverters and
# DataStore both store `enum.name` and re-hydrate via `valueOf(...)`. If R8 renamed
# the constants, every stored SessionType / MuscleGroup / Equipment / EffortRating /
# CardioType / RestReason … would fail to parse and silently drop history. Keep all
# enum members so the persisted strings keep round-tripping.
-keepclassmembers enum * {
    <fields>;
    public static **[] values();
    public static ** valueOf(java.lang.String);
}

# ── Room ──────────────────────────────────────────────────────────────────────
# Room generates code (no runtime reflection for column mapping), but keep the
# entities + projections + DAO-returned data classes explicitly: it's cheap and
# removes a whole class of "release-only" data bugs if a future query relies on
# member names. Schema is exported separately for the migration tests.
-keep class com.forge.app.data.db.entities.** { *; }
-keep class com.forge.app.data.db.projections.** { *; }

# ── WorkManager workers ───────────────────────────────────────────────────────
# Default WorkManager init is disabled in favour of the Hilt worker factory, which
# resolves workers by class — keep every worker so none is stripped/renamed away.
-keep class * extends androidx.work.ListenableWorker { *; }
# HiltWorkerFactory resolves each @HiltWorker through the Dagger graph; keep the annotated
# workers' @AssistedInject constructors and the Hilt-generated assisted-factories so release
# minification can't break worker dependency injection (would NPE on first WorkManager run).
-keep @androidx.hilt.work.HiltWorker class * { <init>(...); }
-keep @dagger.assisted.AssistedFactory class * { *; }

# ── Health Connect ────────────────────────────────────────────────────────────
# The records Forge reads/writes are referenced directly in code (so not stripped),
# but keep the records package defensively against the SDK's record-type reflection.
-keep class androidx.health.connect.client.records.** { *; }

# ── Coroutines (defensive; normally covered by consumer rules) ─────────────────
-keepclassmembers class kotlinx.coroutines.** { volatile <fields>; }
-dontwarn kotlinx.coroutines.**
