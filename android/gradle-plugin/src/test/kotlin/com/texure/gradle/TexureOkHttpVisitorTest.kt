package com.texure.gradle

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.objectweb.asm.ClassReader
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Opcodes
import org.objectweb.asm.tree.ClassNode
import org.objectweb.asm.tree.FieldInsnNode
import org.objectweb.asm.tree.MethodInsnNode

class TexureOkHttpVisitorTest {

    @Test
    fun testExtensionDefaultAndToggle() {
        // Test that TexureExtension defaults to networkMocking = true and can be disabled
        val project = org.gradle.testfixtures.ProjectBuilder.builder().build()
        val extension = project.extensions.create("texure", TexureExtension::class.java)
        assertTrue(extension.networkMocking.get())

        extension.networkMocking.set(false)
        assertFalse(extension.networkMocking.get())

        // Test Kotlin DSL invocation syntax
        extension.networkMocking(true)
        assertTrue(extension.networkMocking.get())
        extension.networkMocking(false)
        assertFalse(extension.networkMocking.get())
    }

    @Test
    fun testIsInstrumentableTargeting() {
        val project = org.gradle.testfixtures.ProjectBuilder.builder().build()
        val factory = object : TexureOkHttpVisitorFactory() {
            override val parameters = project.objects.property(TexureOkHttpParameters::class.java).apply {
                val params = object : TexureOkHttpParameters {
                    override val networkMocking = project.objects.property(Boolean::class.java).convention(true)
                }
                set(params)
            }
            override val instrumentationContext = object : com.android.build.api.instrumentation.InstrumentationContext {
                override val apiVersion = project.objects.property(Int::class.javaObjectType).convention(1)
            }
        }

        val okHttpBuilderClassData = DummyClassData("okhttp3.OkHttpClient\$Builder")
        val otherClassData = DummyClassData("com.example.MyService")

        // With mocking enabled
        assertTrue(factory.isInstrumentable(okHttpBuilderClassData))
        assertFalse(factory.isInstrumentable(otherClassData))

        // With mocking disabled
        factory.parameters.get().networkMocking.set(false)
        assertFalse(factory.isInstrumentable(okHttpBuilderClassData))
    }

    @Test
    fun testOkHttpBuildInstrumentationBytecode() {
        // Create dummy okhttp3/OkHttpClient$Builder class with build() method
        val cw = ClassWriter(ClassWriter.COMPUTE_MAXS or ClassWriter.COMPUTE_FRAMES)
        cw.visit(Opcodes.V1_8, Opcodes.ACC_PUBLIC or Opcodes.ACC_FINAL, "okhttp3/OkHttpClient\$Builder", null, "java/lang/Object", null)

        val mv = cw.visitMethod(Opcodes.ACC_PUBLIC, "build", "()Lokhttp3/OkHttpClient;", null, null)
        mv.visitCode()
        mv.visitTypeInsn(Opcodes.NEW, "okhttp3/OkHttpClient")
        mv.visitInsn(Opcodes.DUP)
        mv.visitVarInsn(Opcodes.ALOAD, 0)
        mv.visitMethodInsn(Opcodes.INVOKESPECIAL, "okhttp3/OkHttpClient", "<init>", "(Lokhttp3/OkHttpClient\$Builder;)V", false)
        mv.visitInsn(Opcodes.ARETURN)
        mv.visitMaxs(3, 1)
        mv.visitEnd()
        cw.visitEnd()

        val originalBytes = cw.toByteArray()

        // Transform with TexureOkHttpClassVisitor
        val cr = ClassReader(originalBytes)
        val targetWriter = ClassWriter(ClassWriter.COMPUTE_MAXS or ClassWriter.COMPUTE_FRAMES)
        val visitor = TexureOkHttpClassVisitor(targetWriter)
        cr.accept(visitor, 0)
        val transformedBytes = targetWriter.toByteArray()

        // Verify bytecode instructions using ClassNode
        val nodeReader = ClassReader(transformedBytes)
        val classNode = ClassNode()
        nodeReader.accept(classNode, 0)

        val buildMethod = classNode.methods.find { it.name == "build" && it.desc == "()Lokhttp3/OkHttpClient;" }
        org.junit.Assert.assertNotNull("build method must exist", buildMethod)

        // Check for TexureNetworkInterceptor reference
        val getStatic = buildMethod!!.instructions.toArray().filterIsInstance<FieldInsnNode>().find {
            it.opcode == Opcodes.GETSTATIC &&
                it.owner == "com/texure/bridge/network/TexureNetworkInterceptor" &&
                it.name == "INSTANCE"
        }
        org.junit.Assert.assertNotNull("GETSTATIC TexureNetworkInterceptor.INSTANCE must be present", getStatic)

        // Check for addInterceptor invocation
        val addInterceptorCall = buildMethod.instructions.toArray().filterIsInstance<MethodInsnNode>().find {
            it.opcode == Opcodes.INVOKEVIRTUAL &&
                it.owner == "okhttp3/OkHttpClient\$Builder" &&
                it.name == "addInterceptor" &&
                it.desc == "(Lokhttp3/Interceptor;)Lokhttp3/OkHttpClient\$Builder;"
        }
        org.junit.Assert.assertNotNull("addInterceptor must be called", addInterceptorCall)

        // Check for interceptors() dedupe list check
        val interceptorsCall = buildMethod.instructions.toArray().filterIsInstance<MethodInsnNode>().find {
            it.opcode == Opcodes.INVOKEVIRTUAL &&
                it.owner == "okhttp3/OkHttpClient\$Builder" &&
                it.name == "interceptors" &&
                it.desc == "()Ljava/util/List;"
        }
        org.junit.Assert.assertNotNull("interceptors() must be called for dedupe guard", interceptorsCall)

        val containsCall = buildMethod.instructions.toArray().filterIsInstance<MethodInsnNode>().find {
            it.opcode == Opcodes.INVOKEINTERFACE &&
                it.owner == "java/util/List" &&
                it.name == "contains" &&
                it.desc == "(Ljava/lang/Object;)Z"
        }
        org.junit.Assert.assertNotNull("contains() must be invoked on interceptors list", containsCall)
    }

    private class DummyClassData(private val name: String) : com.android.build.api.instrumentation.ClassData {
        override val className: String get() = name
        override val classAnnotations: List<String> get() = emptyList()
        override val interfaces: List<String> get() = emptyList()
        override val superClasses: List<String> get() = emptyList()
    }
}
