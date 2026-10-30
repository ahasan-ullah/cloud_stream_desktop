package android.content

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.io.File

/** Binary-compatible subset of android.content.SharedPreferences. */
interface SharedPreferences {
    fun getAll(): Map<String, *>
    fun getString(key: String?, defValue: String?): String?
    fun getStringSet(key: String?, defValues: Set<String>?): Set<String>?
    fun getInt(key: String?, defValue: Int): Int
    fun getLong(key: String?, defValue: Long): Long
    fun getFloat(key: String?, defValue: Float): Float
    fun getBoolean(key: String?, defValue: Boolean): Boolean
    fun contains(key: String?): Boolean
    fun edit(): Editor
    fun registerOnSharedPreferenceChangeListener(listener: OnSharedPreferenceChangeListener?)
    fun unregisterOnSharedPreferenceChangeListener(listener: OnSharedPreferenceChangeListener?)

    interface Editor {
        fun putString(key: String?, value: String?): Editor
        fun putStringSet(key: String?, values: Set<String>?): Editor
        fun putInt(key: String?, value: Int): Editor
        fun putLong(key: String?, value: Long): Editor
        fun putFloat(key: String?, value: Float): Editor
        fun putBoolean(key: String?, value: Boolean): Editor
        fun remove(key: String?): Editor
        fun clear(): Editor
        fun commit(): Boolean
        fun apply()
    }

    fun interface OnSharedPreferenceChangeListener {
        fun onSharedPreferenceChanged(prefs: SharedPreferences?, key: String?)
    }
}

/** SharedPreferences persisted as a small typed JSON file. */
class FileSharedPreferences(private val file: File) : SharedPreferences {
    private val values: MutableMap<String, Any> = load()
    private val listeners = mutableSetOf<SharedPreferences.OnSharedPreferenceChangeListener>()

    @Synchronized
    override fun getAll(): Map<String, *> = values.toMap()

    @Synchronized
    override fun getString(key: String?, defValue: String?) = values[key] as? String ?: defValue

    @Suppress("UNCHECKED_CAST")
    @Synchronized
    override fun getStringSet(key: String?, defValues: Set<String>?) = (values[key] as? Set<String>) ?: defValues

    @Synchronized
    override fun getInt(key: String?, defValue: Int) = (values[key] as? Number)?.toInt() ?: defValue

    @Synchronized
    override fun getLong(key: String?, defValue: Long) = (values[key] as? Number)?.toLong() ?: defValue

    @Synchronized
    override fun getFloat(key: String?, defValue: Float) = (values[key] as? Number)?.toFloat() ?: defValue

    @Synchronized
    override fun getBoolean(key: String?, defValue: Boolean) = values[key] as? Boolean ?: defValue

    @Synchronized
    override fun contains(key: String?) = key in values

    override fun edit(): SharedPreferences.Editor = EditorImpl()

    @Synchronized
    override fun registerOnSharedPreferenceChangeListener(listener: SharedPreferences.OnSharedPreferenceChangeListener?) {
        listener?.let(listeners::add)
    }

    @Synchronized
    override fun unregisterOnSharedPreferenceChangeListener(listener: SharedPreferences.OnSharedPreferenceChangeListener?) {
        listener?.let(listeners::remove)
    }

    private inner class EditorImpl : SharedPreferences.Editor {
        private val changes = mutableMapOf<String, Any?>() // null value = remove
        private var clear = false

        private fun put(key: String?, value: Any?) = apply { if (key != null) changes[key] = value }

        override fun putString(key: String?, value: String?) = put(key, value)
        override fun putStringSet(key: String?, values: Set<String>?) = put(key, values?.toSet())
        override fun putInt(key: String?, value: Int) = put(key, value)
        override fun putLong(key: String?, value: Long) = put(key, value)
        override fun putFloat(key: String?, value: Float) = put(key, value)
        override fun putBoolean(key: String?, value: Boolean) = put(key, value)
        override fun remove(key: String?) = put(key, null)
        override fun clear() = apply { clear = true }

        override fun commit(): Boolean {
            val notify: List<SharedPreferences.OnSharedPreferenceChangeListener>
            synchronized(this@FileSharedPreferences) {
                if (clear) values.clear()
                changes.forEach { (k, v) -> if (v == null) values.remove(k) else values[k] = v }
                runCatching { save() }.onFailure { return false }
                notify = listeners.toList()
            }
            changes.keys.forEach { key -> notify.forEach { it.onSharedPreferenceChanged(this@FileSharedPreferences, key) } }
            return true
        }

        override fun apply() {
            commit()
        }
    }

    private fun load(): MutableMap<String, Any> {
        if (!file.exists()) return mutableMapOf()
        return runCatching {
            Json.parseToJsonElement(file.readText()).jsonObject.mapNotNull { (k, v) -> decode(v)?.let { k to it } }
                .toMap(mutableMapOf())
        }.getOrElse { mutableMapOf() }
    }

    private fun save() {
        file.parentFile?.mkdirs()
        file.writeText(JsonObject(values.mapValues { (_, v) -> encode(v) }).toString())
    }

    // Values carry a type tag so ints stay ints and sets stay sets across restarts.
    private fun encode(v: Any): JsonElement = when (v) {
        is String -> tagged("s", JsonPrimitive(v))
        is Int -> tagged("i", JsonPrimitive(v))
        is Long -> tagged("l", JsonPrimitive(v))
        is Float -> tagged("f", JsonPrimitive(v))
        is Boolean -> tagged("b", JsonPrimitive(v))
        is Set<*> -> tagged("set", JsonArray(v.map { JsonPrimitive(it.toString()) }))
        else -> tagged("s", JsonPrimitive(v.toString()))
    }

    private fun tagged(type: String, value: JsonElement) = JsonObject(mapOf("t" to JsonPrimitive(type), "v" to value))

    private fun decode(e: JsonElement): Any? {
        val obj = e as? JsonObject ?: return null
        val v = obj["v"] ?: return null
        return when (obj["t"]?.jsonPrimitive?.content) {
            "s" -> v.jsonPrimitive.content
            "i" -> v.jsonPrimitive.content.toIntOrNull()
            "l" -> v.jsonPrimitive.content.toLongOrNull()
            "f" -> v.jsonPrimitive.content.toFloatOrNull()
            "b" -> v.jsonPrimitive.booleanOrNull
            "set" -> v.jsonArray.map { it.jsonPrimitive.content }.toSet()
            else -> null
        }
    }
}
