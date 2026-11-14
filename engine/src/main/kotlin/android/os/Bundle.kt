@file:Suppress("unused")
package android.os

/** Minimal stub for android.os.Bundle so Fragment/Activity verification succeeds. */
open class Bundle {
    private val map = mutableMapOf<String, Any?>()

    open fun putString(key: String?, value: String?) { if (key != null) map[key] = value }
    open fun getString(key: String?): String? = map[key] as? String
    open fun putInt(key: String?, value: Int) { if (key != null) map[key] = value }
    open fun getInt(key: String?, defaultValue: Int = 0): Int = (map[key] as? Number)?.toInt() ?: defaultValue
    open fun putBoolean(key: String?, value: Boolean) { if (key != null) map[key] = value }
    open fun getBoolean(key: String?, defaultValue: Boolean = false): Boolean = map[key] as? Boolean ?: defaultValue
    open fun containsKey(key: String?): Boolean = map.containsKey(key)
    open fun clear() { map.clear() }
}
