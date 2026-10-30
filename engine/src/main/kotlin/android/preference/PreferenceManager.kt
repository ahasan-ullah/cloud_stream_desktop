package android.preference

import android.content.Context
import android.content.DesktopContext
import android.content.SharedPreferences

object PreferenceManager {
    @JvmStatic
    fun getDefaultSharedPreferences(context: Context?): SharedPreferences =
        DesktopContext.preferences("com.lagradost.cloudstream3_preferences")
}
