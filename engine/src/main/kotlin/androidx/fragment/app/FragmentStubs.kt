@file:Suppress("unused")
package androidx.fragment.app

import android.content.Context
import android.os.Bundle

/**
 * Minimal shells for Android Fragment types so plugins containing settings fragments
 * can verify and link successfully on the desktop JVM.
 */
open class Fragment {
    open fun getContext(): Context? = null
    open fun getActivity(): androidx.appcompat.app.AppCompatActivity? = null
    open fun getArguments(): Bundle? = null
    open fun setArguments(args: Bundle?) {}
}

open class DialogFragment : Fragment() {
    open fun show(manager: FragmentManager, tag: String?) {}
    open fun showNow(manager: FragmentManager, tag: String?) {}
    open fun dismiss() {}
    open fun dismissAllowingStateLoss() {}
}

open class FragmentManager {
    open fun beginTransaction(): FragmentTransaction = FragmentTransaction()
}

open class FragmentTransaction {
    open fun add(fragment: Fragment, tag: String?): FragmentTransaction = this
    open fun remove(fragment: Fragment): FragmentTransaction = this
    open fun commit(): Int = 0
    open fun commitAllowingStateLoss(): Int = 0
}
