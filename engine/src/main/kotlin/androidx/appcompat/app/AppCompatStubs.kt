@file:Suppress("unused")
package androidx.appcompat.app

import android.app.Activity
import androidx.fragment.app.FragmentManager

/** Minimal stub for AppCompatActivity so plugin classes referencing it can verify and load. */
open class AppCompatActivity : Activity() {
    open fun getSupportFragmentManager(): FragmentManager = FragmentManager()
    open fun isFinishing(): Boolean = false
    open fun isDestroyed(): Boolean = false
}
