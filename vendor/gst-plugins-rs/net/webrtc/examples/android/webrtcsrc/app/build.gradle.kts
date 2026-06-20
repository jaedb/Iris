import java.text.SimpleDateFormat
import java.util.Date

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.jetbrains.kotlin.android)
    alias(libs.plugins.androidx.navigation.safeargs)
    kotlin("plugin.serialization") version "1.9.22"
}

android {
    namespace = "org.freedesktop.gstreamer.examples.webrtcsrc"
    compileSdk = 34
    ndkVersion = "25.2.9519653"

    defaultConfig {
        applicationId = "org.freedesktop.gstreamer.examples.WebRTCSrc"
        minSdk = 28
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"

        externalNativeBuild {
            cmake {
                var gstRoot: String?
                if (project.hasProperty("gstAndroidRoot"))
                    gstRoot = project.property("gstAndroidRoot").toString()
                else
                    gstRoot = System.getenv("GSTREAMER_ROOT_ANDROID")
                if (gstRoot == null)
                    throw GradleException("GSTREAMER_ROOT_ANDROID must be set, or 'gstAndroidRoot' must be defined in your gradle.properties in the top level directory of the unpacked universal GStreamer Android binaries")

                arguments ("-DANDROID_STL=c++_shared", "-DGSTREAMER_ROOT_ANDROID=$gstRoot", "-GNinja")

                targets("gstreamer_webrtcsrc")

                // All archs except MIPS and MIPS64 are supported
                abiFilters("armeabi-v7a", "arm64-v8a", "x86", "x86_64")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }

    applicationVariants.all {
        val variant = this
        variant.outputs
            .map { it as com.android.build.gradle.internal.api.BaseVariantOutputImpl }
            .forEach { output ->
                val date = SimpleDateFormat("YYYYMMdd").format(Date())
                val outputFileName = "GstExamples.WebRTCSrc-${variant.baseName}-${variant.versionName}-${date}.apk"
                output.outputFileName = outputFileName
            }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        viewBinding = true
    }
    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
        }
    }
}

dependencies {
    implementation(libs.androidx.appcompat)
    implementation(libs.androidx.constraintlayout)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.navigation.fragment.ktx)
    implementation(libs.androidx.navigation.ui.ktx)
    implementation(libs.androidx.preference)
    implementation(libs.androidx.recyclerview)
    implementation(libs.androidx.work.runtime.ktx)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.ktor.client.okhttp)
    implementation(libs.material)
}
