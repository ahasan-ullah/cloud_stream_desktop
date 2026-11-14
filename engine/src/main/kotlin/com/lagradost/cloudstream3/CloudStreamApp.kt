package com.lagradost.cloudstream3

import android.app.Activity
import android.content.Context
import android.content.DesktopContext

/**
 * Stand-ins for the Android app's Application/Activity singletons. Plugins mostly use
 * `CloudStreamApp.context` to reach SharedPreferences, and `CommonActivity.activity`
 * to show optional dialogs; there is no activity on desktop, so that stays null.
 */
class CloudStreamApp {
    companion object {
        var context: Context? = DesktopContext

        fun setKey(path: String, value: Any?) {
            com.lagradost.cloudstream3.utils.DataStore.setKey(path, value)
        }

        fun setKey(tag: String, key: String, value: Any?) {
            com.lagradost.cloudstream3.utils.DataStore.setKey(tag, key, value)
        }

        fun <T> getKey(path: String): T? = com.lagradost.cloudstream3.utils.DataStore.getKey(path)

        fun <T> getKey(tag: String, key: String): T? = com.lagradost.cloudstream3.utils.DataStore.getKey(tag, key)

        fun <T> getKey(path: String, def: T): T = com.lagradost.cloudstream3.utils.DataStore.getKey(path) ?: def

        fun removeKey(path: String) {
            com.lagradost.cloudstream3.utils.DataStore.removeKey(path)
        }
    }
}

/** Older plugins still reference the pre-rename Application class. */
class AcraApplication {
    companion object {
        var context: Context? = DesktopContext
    }
}

object CommonActivity {
    var activity: Activity? = null
}
