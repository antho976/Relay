package com.forge.app.core.io

import android.util.AtomicFile
import java.io.File
import java.io.FileOutputStream

internal inline fun File.writeAtomically(write: (FileOutputStream) -> Unit) {
    val atomicFile = AtomicFile(this)
    val output = atomicFile.startWrite()
    try {
        write(output)
        atomicFile.finishWrite(output)
    } catch (failure: Throwable) {
        atomicFile.failWrite(output)
        throw failure
    }
}

internal fun File.writeTextAtomically(text: String) =
    writeAtomically { it.write(text.toByteArray(Charsets.UTF_8)) }

internal fun File.readTextAtomically(): String =
    AtomicFile(this).openRead().bufferedReader(Charsets.UTF_8).use { it.readText() }

internal fun File.existsAtomically(): Boolean = exists() || File("$path.bak").exists()
