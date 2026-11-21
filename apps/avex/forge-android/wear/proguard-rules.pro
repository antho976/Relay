# ─── Avex Wear release R8 / ProGuard rules ───────────────────────────────────
# This file was empty while :wear minifies, so the watch APK kept only what
# proguard-android-optimize.txt and the library consumer rules provide. The phone's
# rules file has kept enum <fields> since the beginning, calling it
# PERSISTENCE-CRITICAL. The two APKs are minified by two independent R8 runs and
# they talk to each other over a wire protocol, so a rule on one end and not the
# other is a release-only interoperability hazard that no debug build and no unit
# test can see: WearCodec.decode reports a failure as DecodeResult.Invalid and drops
# the payload silently. The symptom is "the watch's timer buttons stopped working in
# the Play build", with no crash and no log.

# Readable crash reports (line numbers) without leaking original file names. Watch
# crash traces were unreadable without this.
-keepattributes SourceFile,LineNumberTable
-renamesourcefileattribute SourceFile

# ── Enums (PROTOCOL-CRITICAL) ────────────────────────────────────────────────
# kotlinx.serialization writes an enum as its constant NAME, and two of those cross
# the Data Layer in both directions: ProtocolWeightUnit (phone → watch) and
# TimerCommand.Action (watch → phone). The default AGP file keeps values()/valueOf()
# but not <fields>. Mirrors app/proguard-rules.pro exactly.
-keepclassmembers enum * {
    <fields>;
    public static **[] values();
    public static ** valueOf(java.lang.String);
}

# ── The wire protocol ────────────────────────────────────────────────────────
# Belt and braces alongside the @SerialName literals on the DTOs: keep the shared
# protocol types and their generated serializers whole, so neither end of the
# protocol can be renamed out from under the other.
-keep class com.forge.shared.protocol.** { *; }
-keep class com.forge.shared.weight.** { *; }
-keepclassmembers class com.forge.shared.** {
    *** Companion;
    kotlinx.serialization.KSerializer serializer(...);
}
