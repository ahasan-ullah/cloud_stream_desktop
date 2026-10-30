import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("jvm") version "2.4.0"
    kotlin("plugin.serialization") version "2.4.0"
    id("com.gradleup.shadow") version "9.2.2"
    application
}

group = "com.cloudstream.engine"
version = "0.1.0"

val cloudstreamVersion: String by project
val ktorVersion = "3.5.0"

dependencies {
    // Provider interfaces, NiceHttp and the built-in video host extractors.
    implementation("com.github.recloudstream.cloudstream:library-jvm:$cloudstreamVersion")

    implementation("io.ktor:ktor-server-cio:$ktorVersion")
    implementation("io.ktor:ktor-server-content-negotiation:$ktorVersion")
    implementation("io.ktor:ktor-server-status-pages:$ktorVersion")
    implementation("io.ktor:ktor-serialization-kotlinx-json:$ktorVersion")

    // .cs3 plugins ship Android DEX bytecode; convert it to JVM classes on install.
    implementation("de.femtopedia.dex2jar:dex-translator:2.4.28")

    implementation("org.slf4j:slf4j-simple:2.0.17")
}

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

application {
    mainClass.set("com.cloudstream.engine.MainKt")
}

tasks.shadowJar {
    archiveFileName.set("engine.jar")
    mergeServiceFiles()
}

// Trimmed runtime shipped inside the desktop installer, so users need no Java install.
val jreDir = layout.buildDirectory.dir("jre")
val jreModules = listOf(
    "java.base", "java.desktop", "java.logging", "java.management", "java.naming", "java.net.http",
    "java.scripting", "java.sql", "java.xml", "jdk.crypto.ec", "jdk.unsupported", "jdk.zipfs",
)

tasks.register<Exec>("bundleJre") {
    group = "distribution"
    description = "Builds a minimal Java runtime into build/jre with jlink."
    val exe = if (System.getProperty("os.name").startsWith("Windows")) ".exe" else ""
    val jlink = File(System.getProperty("java.home"), "bin/jlink$exe")
    doFirst { delete(jreDir) }
    commandLine(
        jlink.path, "--add-modules", jreModules.joinToString(","),
        "--strip-debug", "--no-header-files", "--no-man-pages", "--compress=2",
        "--output", jreDir.get().asFile.path,
    )
}
