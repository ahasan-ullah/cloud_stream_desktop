@file:Suppress("unused")
package androidx.compose.runtime

/**
 * Minimal Compose runtime stubs for the desktop JVM engine.
 *
 * CloudStream plugins compiled with Jetpack Compose reference these annotations and
 * interfaces at the bytecode level.  On Android the real Compose runtime is present;
 * on the desktop JVM it is not, so we provide binary-compatible no-op stand-ins so
 * that class loading does not fail with NoClassDefFoundError.
 *
 * None of these are ever *called* – providers only use Compose for their Android UI,
 * not for the scraping logic executed here.
 */

@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.CLASS, AnnotationTarget.FUNCTION, AnnotationTarget.PROPERTY)
annotation class Stable

@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.CLASS, AnnotationTarget.FUNCTION, AnnotationTarget.PROPERTY,
        AnnotationTarget.TYPE, AnnotationTarget.TYPE_PARAMETER)
annotation class Composable

@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.CLASS)
annotation class Immutable

/** Stand-in for `androidx.compose.runtime.State<T>`. */
interface State<T> {
    val value: T
}

/** Stand-in for `androidx.compose.runtime.MutableState<T>`. */
interface MutableState<T> : State<T> {
    override var value: T
}

/** Stub for `androidx.compose.runtime.SnapshotMutationPolicy<T>`. */
interface SnapshotMutationPolicy<T> {
    fun equivalent(a: T, b: T): Boolean
    fun merge(previous: T, current: T, applied: T): T? = null
}

/** Stub for `androidx.compose.runtime.Composer`. Not used by providers. */
interface Composer

/** Stub so plugins that reference `CompositionLocalMap` can still be loaded. */
typealias CompositionLocalMap = Map<CompositionLocal<Any>, Any>

/** Stub for `androidx.compose.runtime.CompositionLocal`. */
abstract class CompositionLocal<T> protected constructor(defaultFactory: () -> T)
