package com.lagradost.cloudstream3.plugins

import android.content.Context
import android.content.res.Resources

/**
 * Mirrors the Android app's `Plugin` class that every .cs3 plugin extends.
 * The real one lives in the Android app module, not in `library`, so the
 * engine provides a binary-compatible copy.
 */
abstract class Plugin : BasePlugin() {
    var resources: Resources? = null
    var openSettings: ((context: Context) -> Unit)? = null

    open fun load(context: Context) {
        load()
    }
}
