package android.os

import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit

/** There is no UI thread on desktop; a single daemon thread plays the main looper. */
class Looper private constructor() {
    companion object {
        private val main = Looper()

        @JvmStatic
        fun getMainLooper(): Looper = main

        @JvmStatic
        fun myLooper(): Looper? = main
    }
}

open class Handler @JvmOverloads constructor(looper: Looper? = null) {
    private val pending = ConcurrentHashMap<Runnable, ScheduledFuture<*>>()

    open fun post(r: Runnable): Boolean = postDelayed(r, 0)

    open fun postDelayed(r: Runnable, delayMillis: Long): Boolean {
        pending[r] = scheduler.schedule({
            pending.remove(r)
            runCatching { r.run() }
        }, delayMillis.coerceAtLeast(0), TimeUnit.MILLISECONDS)
        return true
    }

    open fun removeCallbacks(r: Runnable) {
        pending.remove(r)?.cancel(false)
    }

    open fun removeCallbacksAndMessages(token: Any?) {
        pending.values.forEach { it.cancel(false) }
        pending.clear()
    }

    private companion object {
        val scheduler = Executors.newSingleThreadScheduledExecutor { r ->
            Thread(r, "android-main").apply { isDaemon = true }
        }
    }
}
