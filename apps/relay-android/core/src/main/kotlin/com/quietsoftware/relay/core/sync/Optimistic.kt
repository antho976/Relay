package com.quietsoftware.relay.core.sync

import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.s
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import java.time.Instant

/**
 * What a change will look like once the PC applies it, so the phone shows it at once, with or
 * without the PC. Each rule mirrors what the engine's handler does to the row; the PC's answer
 * replaces the guess when it comes. Ops without a rule are still queued, they just show nothing
 * until the PC answers.
 */
object Optimistic {
    /** Temp ids are `tmp:<outbox entry id>` until the PC names the row. */
    const val TEMP = "tmp:"

    fun tempId(entryId: String) = TEMP + entryId

    fun isTemp(id: String) = id.startsWith(TEMP)

    /** The ops that may wait in the outbox while the PC is away. Anything else needs the PC now. */
    val QUEUEABLE = setOf(
        "task.create", "task.update", "task.move", "task.delete", "task.restore", "task.approve",
        "task.label.add", "task.label.remove", "task.changelog.write", "task.parent.set",
        "task.relate", "task.unrelate", "task.link_commit",
        "notes.create", "notes.update", "notes.pin", "notes.delete", "notes.restore", "notes.append",
        "module.create", "module.update", "module.complete", "module.reopen", "module.delete", "module.restore",
        "notify.ack", "notify.ack_all",
        "mailbox.send",
        "thread.create", "thread.send", "thread.rename", "thread.delete", "thread.set",
        "session.create", "session.spawn",
        "project.update", "workspace.update",
    )

    /**
     * [epochMs] as the PC stamps its rows (time.rs `stamp`): UTC with nine fractional digits, so
     * a row the phone made sorts beside the PC's by its text, not by how many digits it has.
     */
    fun stamp(epochMs: Long): String = STAMP.format(Instant.ofEpochMilli(epochMs))

    private val STAMP = java.time.format.DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm:ss.SSSSSSSSS'Z'").withZone(java.time.ZoneOffset.UTC)

    /** The kind of row an op changes many of at once (Mark all read), with no one [target]. */
    fun broad(op: String): Kind? = if (op == "notify.ack_all") Kind.Notification else null

    /** Whether a [broad] op changes [row], of [kind]. */
    fun covers(op: String, payload: JsonObject, kind: Kind, row: JsonObject): Boolean =
        broad(op) == kind && (payload.s("category") == null || row.s("category") == payload.s("category"))

    /** The row an entry changes, as (kind, id); a create targets its own temp row. */
    fun target(op: String, payload: JsonObject, entryId: String): Pair<Kind, String>? {
        fun idOf(key: String): String? = when (val v = payload[key]) {
            is JsonPrimitive -> v.content
            is JsonObject -> v.s(Refs.KEY)?.let { tempId(it) }
            else -> null
        }
        return when (op) {
            "task.create" -> Kind.Task to tempId(entryId)
            "task.update", "task.move", "task.delete", "task.restore", "task.approve", "task.label.add",
            "task.label.remove", "task.changelog.write", "task.parent.set", "task.relate", "task.unrelate",
            "task.link_commit" -> idOf("task_id")?.let { Kind.Task to it }
            "notes.create" -> Kind.Note to tempId(entryId)
            "notes.update", "notes.pin", "notes.delete", "notes.restore" -> idOf("note_id")?.let { Kind.Note to it }
            "notes.append" -> idOf("note_id")?.let { Kind.Note to it }
            "module.create" -> Kind.Module to tempId(entryId)
            "module.update", "module.complete", "module.reopen", "module.delete", "module.restore" -> idOf("module_id")?.let { Kind.Module to it }
            "notify.ack" -> idOf("notification_id")?.let { Kind.Notification to it }
            "mailbox.send" -> Kind.Mail to tempId(entryId)
            "thread.create" -> Kind.Thread to tempId(entryId)
            "thread.send" -> Kind.Message to tempId(entryId)
            "thread.rename", "thread.delete", "thread.set" -> idOf("id")?.let { Kind.Thread to it }
            "session.create" -> Kind.Session to tempId(entryId)
            "session.spawn" -> idOf("session")?.let { Kind.Session to it }
            "project.update" -> idOf("project_id")?.let { Kind.Project to it }
            "workspace.update" -> idOf("workspace_id")?.let { Kind.Workspace to it }
            else -> null
        }
    }

    /**
     * [row] as it will be after [op]; null when the op removes it from view. [row] is null for a
     * create, which builds its row from the payload.
     */
    fun apply(op: String, payload: JsonObject, row: JsonObject?, entryId: String, now: Long): JsonObject? {
        val ts = stamp(now)
        return when (op) {
            "task.create" -> buildJsonObject {
                put("id", tempId(entryId))
                payload["project_id"]?.let { put("project_id", it) }
                put("title", payload.s("title").orEmpty())
                put("body", payload.s("body").orEmpty())
                put("changelog", payload.s("changelog").orEmpty())
                put("column", payload.s("column") ?: "backlog")
                put("position", Int.MAX_VALUE)
                put("state", payload.s("state") ?: "none")
                put("priority", payload.s("priority") ?: "medium")
                payload.s("size")?.let { put("size", it) }
                put("type", payload.s("type") ?: "task")
                payload["module_id"]?.takeIf { it is JsonPrimitive }?.let { put("module_id", it) }
                payload["parent_id"]?.takeIf { it is JsonPrimitive }?.let { put("parent_id", it) }
                put("depth", 0)
                put("children", JsonArray(emptyList()))
                put("rollup", buildJsonObject { put("total", 0); put("done", 0) })
                put("labels", payload["labels"] as? JsonArray ?: JsonArray(emptyList()))
                put("blocked_by", JsonArray(emptyList()))
                put("blocks", JsonArray(emptyList()))
                put("sessions", JsonArray(emptyList()))
                put("commits", JsonArray(emptyList()))
                put("attachments", JsonArray(emptyList()))
                put("created_at", ts)
                put("updated_at", ts)
            }
            "task.update" -> row?.patch(payload, setOf("title", "body", "priority", "size", "module_id", "state", "changelog", "type"), ts)
            "task.move" -> row?.patch(payload, setOf("column", "position"), ts)
            "task.approve" -> row?.with("column", JsonPrimitive("done"))?.with("state", JsonPrimitive("none"))?.with("updated_at", JsonPrimitive(ts))
            "task.delete", "notes.delete", "module.delete", "thread.delete" -> null
            // A restore undoes a delete the phone made too, so the row shows again at once.
            "task.restore", "notes.restore", "module.restore" -> row?.let { JsonObject(it - Ledger.HIDDEN) }?.with("deleted_at", JsonNull)
            "task.label.add" -> row?.let { r ->
                val label = payload.s("label") ?: return@let r
                val labels = (r["labels"] as? JsonArray).orEmpty().mapNotNull { (it as? JsonPrimitive)?.content }
                if (label in labels) r else r.with("labels", JsonArray((labels + label).map(::JsonPrimitive)))
            }
            "task.label.remove" -> row?.let { r ->
                val label = payload.s("label") ?: return@let r
                val labels = (r["labels"] as? JsonArray).orEmpty().mapNotNull { (it as? JsonPrimitive)?.content }
                r.with("labels", JsonArray(labels.filter { it != label }.map(::JsonPrimitive)))
            }
            "task.changelog.write" -> row?.with("changelog", JsonPrimitive(payload.s("text").orEmpty()))?.with("updated_at", JsonPrimitive(ts))
            "task.parent.set" -> row?.with("parent_id", payload["parent_id"]?.takeIf { it is JsonPrimitive } ?: JsonNull)
            "task.relate", "task.unrelate" -> row?.let { r ->
                val other = (payload["other_id"] as? JsonPrimitive)?.content?.toLongOrNull() ?: return@let r
                val adding = op == "task.relate"
                when (payload.s("relation")) {
                    "blocked_by" -> {
                        val ids = (r["blocked_by"] as? JsonArray).orEmpty().mapNotNull { (it as? JsonPrimitive)?.content?.toLongOrNull() }
                        val next = if (adding) (ids + other).distinct() else ids - other
                        r.with("blocked_by", JsonArray(next.map(::JsonPrimitive)))
                    }
                    "duplicate_of" -> r.with("duplicate_of", if (adding) JsonPrimitive(other) else JsonNull)
                    else -> r
                }
            }
            "task.link_commit" -> row
            "notes.create" -> buildJsonObject {
                put("id", tempId(entryId))
                payload["project_id"]?.let { put("project_id", it) }
                payload.s("title")?.let { put("title", it) }
                put("body", payload.s("body").orEmpty())
                put("pinned", payload["pinned"]?.jsonPrimitive?.content == "true")
                put("created_at", ts)
                put("updated_at", ts)
            }
            "notes.update" -> row?.patch(payload, setOf("title", "body", "pinned"), ts)
            "notes.pin" -> row?.patch(payload, setOf("pinned"), ts)
            "notes.append" -> row?.let { r ->
                val body = r.s("body").orEmpty()
                val text = payload.s("text").orEmpty()
                r.with("body", JsonPrimitive(if (body.isEmpty()) text else "$body\n\n$text")).with("updated_at", JsonPrimitive(ts))
            }
            "module.create" -> buildJsonObject {
                put("id", tempId(entryId))
                payload["project_id"]?.let { put("project_id", it) }
                put("name", payload.s("name").orEmpty())
                put("priority", payload.s("priority") ?: "medium")
                put("order", Int.MAX_VALUE)
                put("created_at", ts)
                put("updated_at", ts)
                put("counts", JsonObject(emptyMap()))
                put("progress_pct", 0)
            }
            "module.update" -> row?.patch(payload, setOf("name", "icon", "priority"), ts)
            "module.complete" -> row?.with("completed_at", JsonPrimitive(ts))
            "module.reopen" -> row?.with("completed_at", JsonNull)
            "notify.ack", "notify.ack_all" -> row?.with("read", JsonPrimitive(true))
            "session.spawn" -> row?.with("state", JsonPrimitive("spawning"))
            "mailbox.send" -> buildJsonObject {
                put("id", tempId(entryId))
                payload["project_id"]?.let { put("project_id", it) }
                put("from", "user")
                put("to", payload.s("to").orEmpty())
                put("text", payload.s("text").orEmpty())
                payload["re_task"]?.takeIf { it is JsonPrimitive }?.let { put("re_task", it) }
                payload["priority"]?.let { put("priority", it) }
                put("sent_at", ts)
            }
            "thread.create" -> buildJsonObject {
                put("id", tempId(entryId))
                put("title", payload.s("text")?.lineSequence()?.firstOrNull()?.take(60)?.ifBlank { null } ?: "New thread")
                put("provider", "claude")
                payload.s("model")?.let { put("model", it) }
                payload.s("effort")?.let { put("effort", it) }
                put("created_at", ts)
                put("updated_at", ts)
                put("working", false)
                put("live", false)
                (payload.s("text")?.ifBlank { null } ?: pictures(images(payload).size))?.let { put("preview", it.take(120)) }
            }
            "thread.send" -> buildJsonObject {
                put("id", tempId(entryId))
                // A thread made on the phone is named by its temp id until the PC answers.
                when (val t = payload["id"]) {
                    is JsonObject -> t.s(Refs.KEY)?.let { put("thread_id", tempId(it)) }
                    null -> Unit
                    else -> put("thread_id", t)
                }
                put("role", "user")
                put("body", messageBody(payload))
                put("created_at", ts)
            }
            "thread.rename" -> row?.patch(payload, setOf("title"), ts)
            "thread.set" -> row?.patch(payload, setOf("model", "effort"), ts)
            "session.create" -> buildJsonObject {
                put("id", -1)
                put("name", tempId(entryId))
                payload["project_id"]?.let { put("project_id", it) }
                put("provider", payload.s("provider") ?: "claude")
                put("role", payload.s("role") ?: "builder")
                payload.s("model")?.let { put("model", it) }
                put("branch", "")
                put("worktree", "")
                put("bus_writes", true)
                put("allow_ui", false)
                put("state", "created")
                put("created_at", ts)
                put("updated_at", ts)
            }
            "project.update" -> row?.patch(payload, setOf("name", "base_branch", "build_cmd", "run_cmd", "pinned"), ts)
            "workspace.update" -> row?.patch(payload, setOf("name"), ts)
            else -> row
        }
    }

    /**
     * The pictures a `thread.send` or `thread.create` names: the ids `thread.upload` gave them on
     * the PC. A picture is never queued itself: it needs the PC to be picked, so a message that
     * waits for the PC names pictures the PC already has.
     */
    private fun images(payload: JsonObject): List<JsonElement> = (payload["images"] as? JsonArray).orEmpty().filter { it is JsonPrimitive && it !is JsonNull }

    /**
     * The person's message as the PC will store it (`{"text", "images"}`), so its bubble shows
     * its pictures while it goes. Only the ids are known here; their size comes with the PC's row.
     */
    private fun messageBody(payload: JsonObject): JsonObject = buildJsonObject {
        put("text", payload.s("text").orEmpty())
        val ids = images(payload)
        if (ids.isNotEmpty()) put("images", JsonArray(ids.map { id -> buildJsonObject { put("id", id) } }))
    }

    /** A thread's preview when its first message is only pictures. */
    private fun pictures(n: Int): String? = when (n) {
        0 -> null
        1 -> "Picture"
        else -> "$n pictures"
    }

    private fun JsonObject.patch(payload: JsonObject, fields: Set<String>, ts: String): JsonObject {
        val out = LinkedHashMap(this)
        for (f in fields) if (payload.containsKey(f)) out[f] = payload[f] ?: JsonNull
        out["updated_at"] = JsonPrimitive(ts)
        return JsonObject(out)
    }

    private fun JsonObject.with(key: String, value: JsonElement): JsonObject = JsonObject(LinkedHashMap(this).also { it[key] = value })

    private fun JsonArray?.orEmpty(): List<JsonElement> = this ?: emptyList()

    /** The temp row an applied create replaces, and the PC's id for it. */
    fun created(op: String, entryId: String, result: JsonElement): Pair<String, String>? {
        if (!op.endsWith(".create") && op != "mailbox.send" && op != "thread.send") return null
        val obj = result as? JsonObject ?: return null
        val real = when (op) {
            "mailbox.send" -> (obj["message"] as? JsonObject)?.l("id")?.toString()
            "session.create" -> obj.s("name")
            else -> obj.l("id")?.toString()
        } ?: return null
        return tempId(entryId) to real
    }

    /** The row(s) an applied op's answer carries, ready for the replica. */
    fun resultRows(op: String, result: JsonElement): List<Row> {
        val obj = result as? JsonObject ?: return emptyList()
        val kind = when {
            op.startsWith("task.") && op != "task.label.list" && obj.containsKey("title") -> Kind.Task
            op.startsWith("notes.") && obj.containsKey("body") -> Kind.Note
            op.startsWith("module.") && obj.containsKey("name") && obj.containsKey("priority") -> null // A Module, not the listed ModuleSummary: the ledger lays it over the listed row
            op == "mailbox.send" -> return listOfNotNull((obj["message"] as? JsonObject)?.let { Row.of(Kind.Mail, it) })
            // thread.list's own rows (ThreadView), as are rename's and set's answers.
            (op == "thread.create" || op == "thread.rename" || op == "thread.set") && obj.containsKey("title") -> Kind.Thread
            op == "thread.send" && obj.containsKey("role") -> Kind.Message
            op.startsWith("session.") && obj.containsKey("name") && obj.containsKey("state") -> Kind.Session
            op == "project.update" && obj.containsKey("path") -> Kind.Project
            op == "workspace.update" && obj.containsKey("path") -> Kind.Workspace
            else -> null
        } ?: return emptyList()
        return listOfNotNull(Row.of(kind, obj))
    }
}
