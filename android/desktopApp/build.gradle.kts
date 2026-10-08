import org.jetbrains.compose.desktop.application.dsl.TargetFormat

plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.compose.multiplatform)
    alias(libs.plugins.kotlin.compose)
}

kotlin {
    sourceSets.main {
        kotlin.srcDir("src/jvmMain/kotlin")
    }
}

dependencies {
    implementation(project(":shared"))
    implementation(compose.desktop.currentOs)
    implementation(libs.kotlinx.coroutines.swing)
}

compose.desktop {
    application {
        mainClass = "com.dscan.desktop.MainKt"

        nativeDistributions {
            targetFormats(TargetFormat.Dmg, TargetFormat.Msi, TargetFormat.Deb)
            packageName = "dscan"
            packageVersion = "1.0.0"
            linux {
                packageVersion = "0.8.1"
                debPackageVersion = "0.8.1"
            }
            macOS {
                packageVersion = "1.0.0"
                dmgPackageVersion = "1.0.0"
            }
            description = "Disk Space Visualizer"
            copyright = "© 2026 AhooraZen"
            vendor = "AhooraZen"
        }
    }
}
