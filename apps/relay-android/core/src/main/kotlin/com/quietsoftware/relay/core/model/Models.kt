package com.quietsoftware.relay.core.model

import com.quietsoftware.relay.core.sync.Ledger.Companion.isHidden
import com.quietsoftware.relay.core.sync.Row
import com.quietsoftware.relay.core.wire.Wire
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.decodeFromJsonElement

/**
 * The bus's entities as the screens read them (docs/engine/BUS.md §11). Decoded leniently from
 * the replica's JSON: a field a newer PC adds is ignored, one an older PC lacks takes its default.
 * Ids are strings because a row the phone made shows under a temp id (`tmp:…`) until the PC
 * answers; [num] is the PC's number when there is one.
 */
inline fun <reified T> Row.decode(): T? = runCatching { Wire.lenient.decodeFromJsonElement<T>(json) }.getOrNull()

inline fun <reified T> List<Row>.decodeAll(): List<T> = filterNot { it.json.isHidden() }.mapNotNull { it.decode<T>() }

inline fun <reified T> JsonElement.decodeAs(): T? = runCatching { Wire.lenient.decodeFromJsonElement<T>(this) }.getOrNull()

private fun String.num(): Long? = toLongOrNull()

@Serializable
data class Workspace(
    val id: String,
    val path: String = "",
    val name: String = "",
    val order: Int = 0,
)

@Serializable
data class Project(
    val id: String,
    @SerialName("workspace_id") val workspaceId: String? = null,
    val path: String = "",
    val name: String = "",
    @SerialName("base_branch") val baseBranch: String = "main",
    @SerialName("build_cmd") val buildCmd: String? = null,
    @SerialName("run_cmd") val runCmd: String? = null,
    val order: Int = 0,
    val pinned: Boolean = false,
) {
    val num: Long? get() = id.num()
}

@Serializable
data class Session(
    val id: Long = 0,
    val name: String,
    val intent: String? = null,
    @SerialName("project_id") val projectId: Long = 0,
    val provider: String = "claude",
    val role: String = "builder",
    val model: String? = null,
    val effort: String? = null,
    val branch: String = "",
    val worktree: String = "",
    @SerialName("task_id") val taskId: Long? = null,
    @SerialName("pair_with") val pairWith: String? = null,
    @SerialName("bus_writes") val busWrites: Boolean = true,
    @SerialName("allow_ui") val allowUi: Boolean = false,
    val state: String = "created",
    val pid: Long? = null,
    @SerialName("exit_code") val exitCode: Int? = null,
    @SerialName("spawned_at") val spawnedAt: String? = null,
    @SerialName("last_output_at") val lastOutputAt: String? = null,
    @SerialName("created_at") val createdAt: String = "",
    @SerialName("updated_at") val updatedAt: String = "",
) {
    /** The lamp a session shows (DESIGN.md "Lamps"). An open shell is not at work, so it is unlit. */
    val lamp: Lamp
        get() = when {
            shell && state in setOf("running", "spawning", "idle") -> Lamp.Off
            state == "running" || state == "spawning" -> Lamp.Live
            state == "blocked" -> Lamp.Held
            state == "restorable" || state == "created" -> Lamp.Waiting
            else -> Lamp.Off
        }

    /** Live enough to have a terminal worth attaching to. */
    val attachable: Boolean get() = state in ATTACHABLE

    /** The owner's own login shell, not an agent: never "working", however long it stays open. */
    val shell: Boolean get() = provider == "shell"

    /** At work on a turn: what every "working" count on the phone counts. */
    val working: Boolean get() = !shell && (state == "running" || state == "spawning")

    /** Stopped on a question or a menu, waiting for the person. */
    val blocked: Boolean get() = state == "blocked"

    /** The word the plate's strip shows; a shell that is up is simply open. */
    val stateLabel: String
        get() = when (state) {
            "running" -> if (shell) "OPEN" else "LIVE"
            "spawning" -> "STARTING"
            "idle" -> if (shell) "OPEN" else "IDLE"
            "blocked" -> "NEEDS YOU"
            "parked" -> "PARKED"
            "restorable" -> "STOPPED"
            "exited" -> "EXITED"
            "created" -> "NOT STARTED"
            else -> state.uppercase()
        }
}

private val ATTACHABLE = setOf("running", "idle", "blocked", "spawning")

/**
 * A session's lamp: [Live] at work, [Held] stopped on the person (drawn in the needs-you amber),
 * [Waiting] parked and ready to start again, [Off] gone.
 */
enum class Lamp { Live, Held, Waiting, Off }

@Serializable
data class Restorable(
    val session: Session,
    val reason: String = "",
    @SerialName("worktree_dirty") val worktreeDirty: Boolean = false,
)

@Serializable
data class Task(
    val id: String,
    @SerialName("project_id") val projectId: Long = 0,
    @SerialName("module_id") val moduleId: Long? = null,
    @SerialName("module_name") val moduleName: String? = null,
    val title: String = "",
    val body: String = "",
    val changelog: String = "",
    val column: String = "backlog",
    val position: Long = 0,
    val state: String = "none",
    val priority: String = "medium",
    val size: String? = null,
    val type: String = "task",
    @SerialName("parent_id") val parentId: Long? = null,
    val depth: Int = 0,
    val children: List<Long> = emptyList(),
    val rollup: Rollup = Rollup(),
    val labels: List<String> = emptyList(),
    @SerialName("blocked_by") val blockedBy: List<Long> = emptyList(),
    val blocks: List<Long> = emptyList(),
    @SerialName("duplicate_of") val duplicateOf: Long? = null,
    val sessions: List<String> = emptyList(),
    val commits: List<Commit> = emptyList(),
    val attachments: List<Attachment> = emptyList(),
    @SerialName("created_at") val createdAt: String = "",
    @SerialName("updated_at") val updatedAt: String = "",
    @SerialName("deleted_at") val deletedAt: String? = null,
) {
    val num: Long? get() = id.num()

    /** `#42`, or `new` for one the PC has not numbered yet. */
    val ref: String get() = num?.let { "#$it" } ?: "new"

    @Serializable
    data class Rollup(val total: Int = 0, val done: Int = 0)

    @Serializable
    data class Commit(val sha: String, val branch: String? = null, @SerialName("linked_at") val linkedAt: String = "")

    @Serializable
    data class Attachment(val id: Long, val name: String = "", val mime: String = "", val bytes: Long = 0)

    companion object {
        val COLUMNS = listOf("backlog", "ready", "active", "in_review", "done")

        fun columnLabel(c: String) = when (c) {
            "backlog" -> "Backlog"
            "ready" -> "Ready"
            "active" -> "Active"
            "in_review" -> "In review"
            "done" -> "Done"
            else -> c
        }
    }
}

@Serializable
data class Module(
    val id: String,
    @SerialName("project_id") val projectId: Long = 0,
    val name: String = "",
    val icon: String? = null,
    val priority: String = "medium",
    val order: Int = 0,
    @SerialName("completed_at") val completedAt: String? = null,
    @SerialName("deleted_at") val deletedAt: String? = null,
    @SerialName("progress_pct") val progressPct: Double = 0.0,
)

@Serializable
data class Note(
    val id: String,
    @SerialName("project_id") val projectId: Long = 0,
    val title: String? = null,
    val body: String = "",
    val pinned: Boolean = false,
    @SerialName("created_at") val createdAt: String = "",
    @SerialName("updated_at") val updatedAt: String = "",
) {
    val num: Long? get() = id.num()

    /** The title, or the body's first line. */
    val heading: String get() = title?.takeIf { it.isNotBlank() } ?: titleOf(body, 80) ?: "Untitled"
}

/**
 * A title from the first line of [text] with words on it, a Markdown heading's `#` taken off, at
 * most [max] characters; null when there is none. A note, a share and a thread made offline are
 * named this way.
 */
fun titleOf(text: String?, max: Int): String? =
    text?.lineSequence()?.map { it.trim().trimStart('#').trim() }?.firstOrNull { it.isNotEmpty() }?.take(max)

@Serializable
data class Hold(
    val id: Long,
    @SerialName("project_id") val projectId: Long? = null,
    val session: String? = null,
    val actor: String = "",
    val op: String = "",
    val policy: String = "",
    val details: JsonElement? = null,
    val state: String = "open",
    @SerialName("created_at") val createdAt: String = "",
)

@Serializable
data class Notification(
    val id: Long,
    @SerialName("project_id") val projectId: Long? = null,
    val category: String = "system",
    val title: String = "",
    val body: String = "",
    val link: JsonElement? = null,
    val read: Boolean = false,
    @SerialName("created_at") val createdAt: String = "",
)

@Serializable
data class Thread(
    val id: String,
    val title: String = "",
    val provider: String = "claude",
    val model: String? = null,
    val effort: String? = null,
    @SerialName("created_at") val createdAt: String = "",
    @SerialName("updated_at") val updatedAt: String = "",
    val working: Boolean = false,
    val live: Boolean = false,
    val preview: String? = null,
    /** `money` (Threads'), `assistant` (Relay's own) or `aide` (an Aide bot's chat, or its Chief of Staff's). */
    val kind: String = "money",
) {
    val num: Long? get() = id.num()
}

@Serializable
data class ThreadMessage(
    val id: String,
    @SerialName("thread_id") val threadId: String = "",
    val role: String = "user",
    val body: JsonElement? = null,
    @SerialName("created_at") val createdAt: String = "",
) {
    /** The text of a user, error or tool message; an assistant's text blocks joined. */
    val text: String
        get() {
            val b = body as? JsonObject ?: return ""
            (b["text"] as? kotlinx.serialization.json.JsonPrimitive)?.let { return it.content }
            val blocks = b["blocks"] as? kotlinx.serialization.json.JsonArray ?: return ""
            return blocks.mapNotNull { blk ->
                val o = blk as? JsonObject ?: return@mapNotNull null
                if ((o["type"] as? kotlinx.serialization.json.JsonPrimitive)?.content == "text") (o["text"] as? kotlinx.serialization.json.JsonPrimitive)?.content else null
            }.joinToString("\n\n")
        }

    /** An assistant message's tool calls, by name. */
    val tools: List<String>
        get() {
            val blocks = (body as? JsonObject)?.get("blocks") as? kotlinx.serialization.json.JsonArray ?: return emptyList()
            return blocks.mapNotNull { blk ->
                val o = blk as? JsonObject ?: return@mapNotNull null
                if ((o["type"] as? kotlinx.serialization.json.JsonPrimitive)?.content == "tool_use") (o["name"] as? kotlinx.serialization.json.JsonPrimitive)?.content else null
            }
        }
}

@Serializable
data class Mail(
    val id: String,
    @SerialName("project_id") val projectId: Long = 0,
    val from: String = "",
    val to: String = "",
    val text: String = "",
    @SerialName("re_task") val reTask: Long? = null,
    val priority: Boolean = false,
    @SerialName("sent_at") val sentAt: String = "",
    @SerialName("acked_at") val ackedAt: String? = null,
)

@Serializable
data class Label(
    val id: Long = 0,
    @SerialName("project_id") val projectId: Long = 0,
    val name: String,
)
