package com.cloudstream.engine

import kotlin.concurrent.thread

/**
 * Returns unused heap to the OS once the engine goes quiet. With SerialGC and a low
 * MaxHeapFreeRatio, a full GC shrinks the committed heap; without one the heap stays at its
 * high-water mark from the last scrape for as long as the app is open.
 */
object IdleTrimmer {
    private const val IDLE_MS = 15_000L

    @Volatile
    private var lastActivity = System.currentTimeMillis()

    @Volatile
    private var dirty = false

    fun touch() {
        lastActivity = System.currentTimeMillis()
        dirty = true
    }

    fun start() {
        thread(isDaemon = true, name = "idle-trim", priority = Thread.MIN_PRIORITY) {
            while (true) {
                Thread.sleep(5_000)
                if (dirty && System.currentTimeMillis() - lastActivity > IDLE_MS) {
                    dirty = false
                    System.gc()
                }
            }
        }
    }
}
