package android.app

import android.content.Context
import android.view.Window

// Minimal shells so plugin classes that mention these types still verify and load.
// They are never instantiated: CommonActivity.activity is always null on desktop.

abstract class Activity : Context()

open class Dialog(context: Context?) {
    open fun getWindow(): Window? = null
    open fun requestWindowFeature(featureId: Int): Boolean = false
    open fun show() {}
    open fun dismiss() {}
}
