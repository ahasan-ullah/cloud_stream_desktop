package android.content

import java.io.File
import java.util.concurrent.ConcurrentHashMap

/**
 * Desktop stand-in for Android's Context. Plugins receive one in `load(context)`;
 * most only register providers, some also keep settings in SharedPreferences.
 */
abstract class Context {
    open fun getSharedPreferences(name: String?, mode: Int): SharedPreferences =
        DesktopContext.preferences(name ?: "default")

    open fun getFilesDir(): File = File(DesktopContext.dataDir, "files").apply { mkdirs() }

    open fun getCacheDir(): File = File(DesktopContext.dataDir, "cache").apply { mkdirs() }

    open fun getPackageName(): String = "com.lagradost.cloudstream3"

    open fun getApplicationContext(): Context = DesktopContext

    open fun getResources(): android.content.res.Resources = android.content.res.Resources()

    /** No activities to start on desktop. */
    open fun startActivity(intent: Intent?) {}

    companion object {
        const val MODE_PRIVATE = 0
    }
}

object DesktopContext : Context() {
    /** Set by the engine at startup; holds per-plugin preference files. */
    @Volatile
    var dataDir: File = File(System.getProperty("java.io.tmpdir"), "cloudstream-engine")

    private val prefs = ConcurrentHashMap<String, FileSharedPreferences>()

    fun preferences(name: String): SharedPreferences = prefs.computeIfAbsent(name) {
        val safe = name.replace(Regex("[^A-Za-z0-9._-]"), "_")
        FileSharedPreferences(File(File(dataDir, "prefs"), "$safe.json"))
    }
}
