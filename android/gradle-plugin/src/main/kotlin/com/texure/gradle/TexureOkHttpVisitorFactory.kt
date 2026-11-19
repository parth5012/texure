package com.texure.gradle

import com.android.build.api.instrumentation.AsmClassVisitorFactory
import com.android.build.api.instrumentation.ClassContext
import com.android.build.api.instrumentation.ClassData
import com.android.build.api.instrumentation.InstrumentationParameters
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.Label
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes

/**
 * Parameters for [TexureOkHttpVisitorFactory].
 */
interface TexureOkHttpParameters : InstrumentationParameters {
    @get:Input
    val networkMocking: Property<Boolean>
}

/**
 * AGP ASM Class Visitor Factory that instruments OkHttpClient.Builder.build()
 * to automatically register TexureNetworkInterceptor in debug builds.
 */
abstract class TexureOkHttpVisitorFactory : AsmClassVisitorFactory<TexureOkHttpParameters> {

    override fun createClassVisitor(
        classContext: ClassContext,
        nextClassVisitor: ClassVisitor,
    ): ClassVisitor {
        return TexureOkHttpClassVisitor(nextClassVisitor)
    }

    override fun isInstrumentable(classData: ClassData): Boolean {
        if (!parameters.get().networkMocking.getOrElse(true)) {
            return false
        }
        val name = classData.className
        return name == "okhttp3.OkHttpClient\$Builder" || name == "okhttp3/OkHttpClient\$Builder"
    }
}

/**
 * ASM ClassVisitor targeting okhttp3.OkHttpClient$Builder.
 */
class TexureOkHttpClassVisitor(
    nextVisitor: ClassVisitor,
) : ClassVisitor(Opcodes.ASM9, nextVisitor) {

    override fun visitMethod(
        access: Int,
        name: String?,
        descriptor: String?,
        signature: String?,
        exceptions: Array<out String>?,
    ): MethodVisitor {
        val mv = super.visitMethod(access, name, descriptor, signature, exceptions)
        if (name == "build" && descriptor == "()Lokhttp3/OkHttpClient;") {
            return TexureOkHttpBuildMethodVisitor(mv)
        }
        return mv
    }
}

/**
 * ASM MethodVisitor that injects TexureNetworkInterceptor.INSTANCE registration
 * into OkHttpClient.Builder.build().
 */
class TexureOkHttpBuildMethodVisitor(
    mv: MethodVisitor,
) : MethodVisitor(Opcodes.ASM9, mv) {

    override fun visitCode() {
        super.visitCode()

        val alreadyAddedLabel = Label()

        // 1. Check if this.interceptors().contains(TexureNetworkInterceptor.INSTANCE)
        mv.visitVarInsn(Opcodes.ALOAD, 0)
        mv.visitMethodInsn(
            Opcodes.INVOKEVIRTUAL,
            "okhttp3/OkHttpClient\$Builder",
            "interceptors",
            "()Ljava/util/List;",
            false,
        )
        mv.visitFieldInsn(
            Opcodes.GETSTATIC,
            INTERCEPTOR_OWNER,
            "INSTANCE",
            INTERCEPTOR_DESC,
        )
        mv.visitMethodInsn(
            Opcodes.INVOKEINTERFACE,
            "java/util/List",
            "contains",
            "(Ljava/lang/Object;)Z",
            true,
        )
        mv.visitJumpInsn(Opcodes.IFNE, alreadyAddedLabel)

        // 2. Add interceptor: this.addInterceptor(TexureNetworkInterceptor.INSTANCE)
        mv.visitVarInsn(Opcodes.ALOAD, 0)
        mv.visitFieldInsn(
            Opcodes.GETSTATIC,
            INTERCEPTOR_OWNER,
            "INSTANCE",
            INTERCEPTOR_DESC,
        )
        mv.visitMethodInsn(
            Opcodes.INVOKEVIRTUAL,
            "okhttp3/OkHttpClient\$Builder",
            "addInterceptor",
            "(Lokhttp3/Interceptor;)Lokhttp3/OkHttpClient\$Builder;",
            false,
        )
        mv.visitInsn(Opcodes.POP)

        // 3. Mark continuation label
        mv.visitLabel(alreadyAddedLabel)
    }

    override fun visitMaxs(maxStack: Int, maxLocals: Int) {
        super.visitMaxs(maxOf(maxStack, 4), maxOf(maxLocals, 1))
    }

    companion object {
        const val INTERCEPTOR_OWNER = "com/texure/bridge/network/TexureNetworkInterceptor"
        const val INTERCEPTOR_DESC = "Lcom/texure/bridge/network/TexureNetworkInterceptor;"
    }
}
