package com.tally.app.data.db

import androidx.room.RoomDatabase
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase

/**
 * What lets this ledger sync with the PC (docs/MONEY.md, "Sync"): a permanent uid and a change
 * time on every row, and a tombstone for every delete. The database keeps them itself, with
 * triggers, so no write path in the app can forget to: an insert or an update stamps the row with
 * the time, a delete (cascades included) leaves a tombstone, and a row's uid never changes.
 *
 * Every trigger but the delete one stands aside while a sync applies the PC's rows (the
 * "applying" row in sync_state), so those keep the PC's change time and uid.
 */
object SyncSchema {

    /**
     * The tables version 3 synced. MIGRATION_2_3 works on exactly these: a table that came later
     * does not exist yet when it runs.
     */
    private val TABLES_V3 = listOf("accounts", "categories", "recurring", "goals", "transactions", "budgets", "goal_contributions", "account_values")

    /** The investment tables version 4 added (docs/INVESTMENTS.md), what they refer to first. */
    private val TABLES_V4 = listOf("securities", "holdings", "activities", "prices", "fx_rates", "room_facts")

    /** The synced Room tables, by their SQL name, in the order the PC applies them. */
    val TABLES = TABLES_V3 + TABLES_V4

    /*
     * Android builds SQLite with recursive triggers on, so a trigger's own write can fire it
     * again. Each WHEN below is false for the write its body makes, which ends the recursion.
     */

    /** Epoch millis inside SQLite. One value for a whole statement, its triggers included. */
    private const val NOW = "CAST((julianday('now') - 2440587.5) * 86400000.0 AS INTEGER)"

    private const val LOCAL = "NOT EXISTS (SELECT 1 FROM sync_state WHERE key = 'applying')"

    /** A random UUID v4 in SQL, for rows that existed before uids did. */
    private const val UUID_SQL =
        "lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))), 2) || '-' || " +
            "substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))), 2) || '-' || lower(hex(randomblob(6)))"

    /** The triggers for [table], by name and SQL, so a fresh database and a migrated one get the same set. */
    private fun triggers(table: String): List<String> {
        // One past the stamp of the tombstone a re-created row had, or 0.
        val buried = "COALESCE((SELECT deletedAt + 1 FROM sync_tombstones WHERE tableName = '$table' AND uid = NEW.uid), 0)"
        // A budget is synced by its category, so its tombstone names it. One whose category is
        // already gone says nothing: the PC drops it with the category's own tombstone.
        val tombstone = if (table == "budgets") {
            """CREATE TRIGGER IF NOT EXISTS budgets_sync_delete AFTER DELETE ON budgets
               WHEN OLD.categoryId = 0 OR EXISTS (SELECT 1 FROM categories WHERE id = OLD.categoryId)
               BEGIN
                 INSERT OR REPLACE INTO sync_tombstones (tableName, uid, deletedAt, category)
                 VALUES ('budgets', OLD.uid, MAX($NOW, OLD.updatedAt + 1), (SELECT uid FROM categories WHERE id = OLD.categoryId));
               END"""
        } else {
            """CREATE TRIGGER IF NOT EXISTS ${table}_sync_delete AFTER DELETE ON $table
               BEGIN
                 INSERT OR REPLACE INTO sync_tombstones (tableName, uid, deletedAt, category)
                 VALUES ('$table', OLD.uid, MAX($NOW, OLD.updatedAt + 1), NULL);
               END"""
        }
        return listOf(
            // A new row is new now, and a row that comes back (an Undo) is no longer deleted:
            // stamped past its own tombstone, so the coming back outranks the delete everywhere.
            """CREATE TRIGGER IF NOT EXISTS ${table}_sync_insert AFTER INSERT ON $table WHEN $LOCAL
               BEGIN
                 UPDATE $table SET updatedAt = MAX($NOW, $buried)
                 WHERE id = NEW.id AND updatedAt < MAX($NOW, $buried);
                 DELETE FROM sync_tombstones WHERE tableName = '$table' AND uid = NEW.uid;
               END""",
            // An edit is newer than both the clock and the version it replaced, even when the
            // writer handed back a stale copy of the row.
            """CREATE TRIGGER IF NOT EXISTS ${table}_sync_update AFTER UPDATE ON $table
               WHEN $LOCAL AND NEW.updatedAt < MAX($NOW, OLD.updatedAt + 1)
               BEGIN
                 UPDATE $table SET updatedAt = MAX($NOW, OLD.updatedAt + 1) WHERE id = NEW.id;
               END""",
            // An editor that rebuilds a row from its fields carries a fresh default uid; the
            // row keeps the one it has. Android's SQLite runs triggers recursively, so the
            // restoring write raises a flag this trigger waits out instead of undoing it again.
            """CREATE TRIGGER IF NOT EXISTS ${table}_sync_uid AFTER UPDATE OF uid ON $table
               WHEN NEW.uid <> OLD.uid AND NOT EXISTS (SELECT 1 FROM sync_state WHERE key IN ('applying', 'keeping_uid'))
               BEGIN
                 INSERT OR REPLACE INTO sync_state (key, value) VALUES ('keeping_uid', 1);
                 UPDATE $table SET uid = OLD.uid WHERE id = NEW.id;
                 DELETE FROM sync_state WHERE key = 'keeping_uid';
               END""",
            tombstone,
        )
    }

    private val SUFFIXES = listOf("sync_insert", "sync_update", "sync_uid", "sync_delete")

    fun createTriggers(db: SupportSQLiteDatabase, tables: List<String> = TABLES) {
        tables.forEach { table -> triggers(table).forEach(db::execSQL) }
    }

    /**
     * Drops and re-creates every sync trigger, so a database always runs the definitions of the
     * app that opened it, whatever an earlier build of the same schema version created.
     */
    fun refreshTriggers(db: SupportSQLiteDatabase) {
        db.beginTransaction()
        try {
            TABLES.forEach { table -> SUFFIXES.forEach { db.execSQL("DROP TRIGGER IF EXISTS ${table}_$it") } }
            createTriggers(db)
            db.setTransactionSuccessful()
        } finally {
            db.endTransaction()
        }
    }

    /** Fresh installs: Room creates the tables, this adds the triggers; every open brings them up to date. */
    val callback = object : RoomDatabase.Callback() {
        override fun onCreate(db: SupportSQLiteDatabase) = createTriggers(db)
        override fun onOpen(db: SupportSQLiteDatabase) = refreshTriggers(db)
    }

    /**
     * 3: uids, change times and tombstones. Every existing row gets a random uid and "now" as its
     * change time, so the first sync carries all of it.
     */
    val MIGRATION_2_3 = object : Migration(2, 3) {
        override fun migrate(db: SupportSQLiteDatabase) {
            TABLES_V3.forEach { table ->
                db.execSQL("ALTER TABLE `$table` ADD COLUMN `uid` TEXT NOT NULL DEFAULT ''")
                db.execSQL("ALTER TABLE `$table` ADD COLUMN `updatedAt` INTEGER NOT NULL DEFAULT 0")
                db.execSQL("UPDATE `$table` SET uid = $UUID_SQL, updatedAt = $NOW")
                db.execSQL("CREATE UNIQUE INDEX IF NOT EXISTS `index_${table}_uid` ON `$table` (`uid`)")
            }
            db.execSQL(
                "CREATE TABLE IF NOT EXISTS `sync_tombstones` (`tableName` TEXT NOT NULL, `uid` TEXT NOT NULL, " +
                    "`deletedAt` INTEGER NOT NULL, `category` TEXT, PRIMARY KEY(`tableName`, `uid`))"
            )
            db.execSQL("CREATE INDEX IF NOT EXISTS `index_sync_tombstones_deletedAt` ON `sync_tombstones` (`deletedAt`)")
            db.execSQL("CREATE TABLE IF NOT EXISTS `sync_state` (`key` TEXT NOT NULL, `value` INTEGER NOT NULL, PRIMARY KEY(`key`))")
            createTriggers(db, TABLES_V3)
        }
    }

    /**
     * 4: investments (docs/INVESTMENTS.md). Accounts gain a registration, an institution and the
     * institution's number, read as none and empty on every account there is; six new tables
     * come with their indices and triggers. The statements are the ones Room writes for the
     * entities (schemas/4.json), so a migrated ledger and a fresh install are the same database.
     */
    val MIGRATION_3_4 = object : Migration(3, 4) {
        override fun migrate(db: SupportSQLiteDatabase) {
            db.execSQL("ALTER TABLE `accounts` ADD COLUMN `registration` TEXT")
            db.execSQL("ALTER TABLE `accounts` ADD COLUMN `institution` TEXT NOT NULL DEFAULT ''")
            db.execSQL("ALTER TABLE `accounts` ADD COLUMN `externalRef` TEXT NOT NULL DEFAULT ''")
            db.execSQL(
                "CREATE TABLE IF NOT EXISTS `securities` (`id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, `symbol` TEXT NOT NULL, " +
                    "`name` TEXT NOT NULL, `currency` TEXT NOT NULL, `kind` TEXT NOT NULL, `exchange` TEXT NOT NULL, " +
                    "`uid` TEXT NOT NULL, `updatedAt` INTEGER NOT NULL)"
            )
            db.execSQL("CREATE UNIQUE INDEX IF NOT EXISTS `index_securities_uid` ON `securities` (`uid`)")
            db.execSQL(
                "CREATE TABLE IF NOT EXISTS `holdings` (`id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, `accountId` INTEGER NOT NULL, " +
                    "`securityId` INTEGER NOT NULL, `date` INTEGER NOT NULL, `quantity` INTEGER NOT NULL, `book` INTEGER NOT NULL, " +
                    "`bookMarket` INTEGER NOT NULL, `uid` TEXT NOT NULL, `updatedAt` INTEGER NOT NULL, " +
                    "FOREIGN KEY(`accountId`) REFERENCES `accounts`(`id`) ON UPDATE NO ACTION ON DELETE CASCADE , " +
                    "FOREIGN KEY(`securityId`) REFERENCES `securities`(`id`) ON UPDATE NO ACTION ON DELETE CASCADE )"
            )
            db.execSQL("CREATE INDEX IF NOT EXISTS `index_holdings_accountId_date` ON `holdings` (`accountId`, `date`)")
            db.execSQL("CREATE INDEX IF NOT EXISTS `index_holdings_securityId` ON `holdings` (`securityId`)")
            db.execSQL("CREATE UNIQUE INDEX IF NOT EXISTS `index_holdings_uid` ON `holdings` (`uid`)")
            db.execSQL(
                "CREATE TABLE IF NOT EXISTS `activities` (`id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, `accountId` INTEGER NOT NULL, " +
                    "`securityId` INTEGER, `type` TEXT NOT NULL, `date` INTEGER NOT NULL, `quantity` INTEGER NOT NULL, " +
                    "`amount` INTEGER NOT NULL, `fee` INTEGER NOT NULL, `currency` TEXT NOT NULL, `toAmount` INTEGER, " +
                    "`toCurrency` TEXT, `note` TEXT NOT NULL, `source` TEXT NOT NULL, `createdAt` INTEGER NOT NULL, " +
                    "`uid` TEXT NOT NULL, `updatedAt` INTEGER NOT NULL, " +
                    "FOREIGN KEY(`accountId`) REFERENCES `accounts`(`id`) ON UPDATE NO ACTION ON DELETE CASCADE , " +
                    "FOREIGN KEY(`securityId`) REFERENCES `securities`(`id`) ON UPDATE NO ACTION ON DELETE SET NULL )"
            )
            db.execSQL("CREATE INDEX IF NOT EXISTS `index_activities_accountId_date` ON `activities` (`accountId`, `date`)")
            db.execSQL("CREATE INDEX IF NOT EXISTS `index_activities_securityId` ON `activities` (`securityId`)")
            db.execSQL("CREATE UNIQUE INDEX IF NOT EXISTS `index_activities_uid` ON `activities` (`uid`)")
            db.execSQL(
                "CREATE TABLE IF NOT EXISTS `prices` (`id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, `securityId` INTEGER NOT NULL, " +
                    "`date` INTEGER NOT NULL, `price` INTEGER NOT NULL, `source` TEXT NOT NULL, `uid` TEXT NOT NULL, " +
                    "`updatedAt` INTEGER NOT NULL, " +
                    "FOREIGN KEY(`securityId`) REFERENCES `securities`(`id`) ON UPDATE NO ACTION ON DELETE CASCADE )"
            )
            db.execSQL("CREATE INDEX IF NOT EXISTS `index_prices_securityId_date` ON `prices` (`securityId`, `date`)")
            db.execSQL("CREATE UNIQUE INDEX IF NOT EXISTS `index_prices_uid` ON `prices` (`uid`)")
            db.execSQL(
                "CREATE TABLE IF NOT EXISTS `fx_rates` (`id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, `base` TEXT NOT NULL, " +
                    "`quote` TEXT NOT NULL, `date` INTEGER NOT NULL, `rate` INTEGER NOT NULL, `source` TEXT NOT NULL, " +
                    "`uid` TEXT NOT NULL, `updatedAt` INTEGER NOT NULL)"
            )
            db.execSQL("CREATE UNIQUE INDEX IF NOT EXISTS `index_fx_rates_uid` ON `fx_rates` (`uid`)")
            db.execSQL(
                "CREATE TABLE IF NOT EXISTS `room_facts` (`id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, `registration` TEXT NOT NULL, " +
                    "`year` INTEGER NOT NULL, `amount` INTEGER NOT NULL, `uid` TEXT NOT NULL, `updatedAt` INTEGER NOT NULL)"
            )
            db.execSQL("CREATE UNIQUE INDEX IF NOT EXISTS `index_room_facts_uid` ON `room_facts` (`uid`)")
            createTriggers(db, TABLES_V4)
        }
    }
}

/** The one place a [TallyDatabase] builder is finished, so the app and every test get the same database. */
fun RoomDatabase.Builder<TallyDatabase>.tally(): RoomDatabase.Builder<TallyDatabase> =
    addMigrations(SyncSchema.MIGRATION_2_3, SyncSchema.MIGRATION_3_4).addCallback(SyncSchema.callback)
