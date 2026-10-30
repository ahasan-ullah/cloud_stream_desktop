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
