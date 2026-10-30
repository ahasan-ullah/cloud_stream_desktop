package android.util

import org.slf4j.LoggerFactory

/** android.util.Log routed to the engine's log output. */
object Log {
    const val VERBOSE = 2
    const val DEBUG = 3
    const val INFO = 4
    const val WARN = 5
    const val ERROR = 6

    private fun logger(tag: String?) = LoggerFactory.getLogger("plugin:${tag ?: "?"}")

    @JvmStatic fun v(tag: String?, msg: String?): Int = 0.also { logger(tag).trace(msg) }
    @JvmStatic fun v(tag: String?, msg: String?, tr: Throwable?): Int = 0.also { logger(tag).trace(msg, tr) }
    @JvmStatic fun d(tag: String?, msg: String?): Int = 0.also { logger(tag).debug(msg) }
    @JvmStatic fun d(tag: String?, msg: String?, tr: Throwable?): Int = 0.also { logger(tag).debug(msg, tr) }
    @JvmStatic fun i(tag: String?, msg: String?): Int = 0.also { logger(tag).info(msg) }
    @JvmStatic fun i(tag: String?, msg: String?, tr: Throwable?): Int = 0.also { logger(tag).info(msg, tr) }
    @JvmStatic fun w(tag: String?, msg: String?): Int = 0.also { logger(tag).warn(msg) }
    @JvmStatic fun w(tag: String?, msg: String?, tr: Throwable?): Int = 0.also { logger(tag).warn(msg, tr) }
    @JvmStatic fun w(tag: String?, tr: Throwable?): Int = 0.also { logger(tag).warn("", tr) }
    @JvmStatic fun e(tag: String?, msg: String?): Int = 0.also { logger(tag).error(msg) }
    @JvmStatic fun e(tag: String?, msg: String?, tr: Throwable?): Int = 0.also { logger(tag).error(msg, tr) }
    @JvmStatic fun wtf(tag: String?, msg: String?): Int = e(tag, msg)
    @JvmStatic fun isLoggable(tag: String?, level: Int): Boolean = level >= INFO
    @JvmStatic fun getStackTraceString(tr: Throwable?): String = tr?.stackTraceToString().orEmpty()
}
