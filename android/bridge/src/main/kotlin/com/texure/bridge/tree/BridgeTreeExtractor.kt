package com.texure.bridge.tree

import android.app.Activity
import android.view.View
import android.view.ViewGroup
import android.widget.EditText
import android.widget.TextView
import com.texure.bridge.model.TreeNode
import java.util.concurrent.atomic.AtomicLong

class BridgeTreeExtractor {

    private val revCounter = AtomicLong(1)

    fun getRev(): Long = revCounter.get()

    fun bumpRev(): Long = revCounter.incrementAndGet()

    fun extractTree(
        activity: Activity?,
        subtreeId: String? = null,
        maxDepth: Int = DEFAULT_MAX_DEPTH
    ): ExtractedTree {
        if (activity == null) {
            return ExtractedTree(
                rev = revCounter.get(),
                window = "none",
                nodes = emptyList(),
                resolvedViews = emptyList()
            )
        }

        val decorView = activity.window.decorView
        val windowName = "${activity.packageName}/${activity.javaClass.name}"
        val density = activity.resources.displayMetrics.density

        val collectedNodes = mutableListOf<TreeNode>()
        val collectedViews = mutableListOf<View>()

        val rootTarget = if (!subtreeId.isNullOrEmpty()) {
            findViewByTreeId(decorView, subtreeId) ?: return ExtractedTree(
                rev = revCounter.get(),
                window = windowName,
                nodes = emptyList(),
                resolvedViews = emptyList(),
                subtreeNotFound = true
            )
        } else {
            decorView
        }

        // Preserve full ancestry so subtree hashes resolve against full-tree verify (#29 B2).
        val ancestorPath = if (rootTarget === decorView) {
            emptyList()
        } else {
            ancestorPathTo(decorView, rootTarget)
        }

        traverseView(
            view = rootTarget,
            currentDepth = 0,
            maxDepth = maxDepth,
            density = density,
            collectedNodes = collectedNodes,
            collectedViews = collectedViews,
            ancestorPath = ancestorPath
        )

        return ExtractedTree(
            rev = revCounter.get(),
            window = windowName,
            nodes = collectedNodes,
            resolvedViews = collectedViews
        )
    }

    private fun traverseView(
        view: View,
        currentDepth: Int,
        maxDepth: Int,
        density: Float,
        collectedNodes: MutableList<TreeNode>,
        collectedViews: MutableList<View>,
        ancestorPath: List<String>
    ) {
        if (view.visibility != View.VISIBLE || currentDepth > maxDepth) {
            return
        }

        val isInteresting = isNodeInteresting(view)
        if (isInteresting) {
            val location = IntArray(2)
            view.getLocationOnScreen(location)
            val xDp = (location[0] / density).toInt()
            val yDp = (location[1] / density).toInt()
            val wDp = (view.width / density).toInt()
            val hDp = (view.height / density).toInt()

            val text = when (view) {
                is TextView -> view.text?.toString()?.takeIf { it.isNotBlank() }
                else -> null
            }

            val cd = view.contentDescription?.toString()?.takeIf { it.isNotBlank() }
            val id = resolveNodeId(view)
            val isClickable = view.isClickable
            val isEditable = view is EditText || (view.isFocusable && view.isFocusableInTouchMode)

            val treeNode = TreeNode(
                id = id,
                text = text,
                cd = cd,
                cls = view.javaClass.name,
                bounds = listOf(xDp, yDp, wDp, hDp),
                clickable = isClickable,
                editable = isEditable,
                hash = computeSemanticHash(ancestorPath, id, text, view.javaClass.name)
            )
            collectedNodes.add(treeNode)
            collectedViews.add(view)
        }

        if (view is ViewGroup) {
            val childPath = ancestorPath + (resolveNodeId(view) ?: view.javaClass.name)
            for (i in 0 until view.childCount) {
                traverseView(
                    view = view.getChildAt(i),
                    currentDepth = currentDepth + 1,
                    maxDepth = maxDepth,
                    density = density,
                    collectedNodes = collectedNodes,
                    collectedViews = collectedViews,
                    ancestorPath = childPath
                )
            }
        }
    }

    private fun isNodeInteresting(view: View): Boolean {
        if (view.isClickable) return true
        if (view is EditText) return true
        if (view.isFocusable && view.isFocusableInTouchMode) return true
        if (view.isScrollContainer) return true
        if (view is TextView && !view.text.isNullOrBlank()) return true
        if (!view.contentDescription.isNullOrBlank()) return true
        if (view.tag != null) return true
        return false
    }

    fun resolveNodeId(view: View): String? {
        val tag = view.tag?.toString()?.takeIf { it.isNotBlank() }
        if (tag != null) return tag

        val resId = view.id
        if (resId != View.NO_ID && resId > 0) {
            try {
                val entryName = view.resources.getResourceEntryName(resId)
                if (!entryName.isNullOrBlank()) return entryName
            } catch (_: Exception) {}
        }

        return null
    }

    private fun findViewByTreeId(root: View, targetId: String): View? {
        if (resolveNodeId(root) == targetId) return root
        if (root is ViewGroup) {
            for (i in 0 until root.childCount) {
                val found = findViewByTreeId(root.getChildAt(i), targetId)
                if (found != null) return found
            }
        }
        return null
    }

    /**
     * Ancestor-path prefix (root-first, exclusive of [target]) so subtree
     * hashes match their full-tree counterparts (#29). Walks View parents
     * from [target] up to [decorView]; each segment is tag/resource id or
     * class name, matching `traverseView` childPath construction.
     */
    private fun ancestorPathTo(decorView: View, target: View): List<String> {
        val segments = mutableListOf<String>()
        var current: View? = target.parent as? View
        while (current != null && current !== decorView) {
            segments.add(resolveNodeId(current) ?: current.javaClass.name)
            current = current.parent as? View
        }
        // decorView is the full-tree root; subtree mode needs every ancestor
        // strictly above target, root-first. The chain stops at decorView
        // (exclusive), so the decor segment is always included (direct
        // children of decorView have prefix [seg(decor)]).
        segments.reverse()
        return listOf(resolveNodeId(decorView) ?: decorView.javaClass.name) + segments
    }

    companion object {
        const val DEFAULT_MAX_DEPTH = 12

        /**
         * FNV-1a 64-bit semantic hash, hex-encoded (map #36, #29). Inputs are
         * the ancestor path (root-first), the stable tag/resource id, a text
         * snippet, and the class name. Bounds, traversal index, and rev are
         * deliberately excluded: they shift under non-structural updates and
         * would rotate the hash spuriously across runs. Pure Kotlin/JVM
         * arithmetic only; deterministic across runs and processes.
         */
        fun computeSemanticHash(
            ancestorPath: List<String>,
            id: String?,
            text: String?,
            cls: String
        ): String {
            val input = buildString {
                append(ancestorPath.joinToString("/"))
                append('\n')
                append(id ?: "")
                append('\n')
                append((text ?: "").trim().take(1024))
                append('\n')
                append(cls)
            }
            var h: Long = -3750763034362895579L // FNV offset basis 14695981039346656037 as signed Long
            val prime: Long = 1099511628211L
            for (b in input.encodeToByteArray()) {
                h = h xor (b.toLong() and 0xFFL)
                h *= prime
            }
            return java.lang.Long.toUnsignedString(h, 16).padStart(16, '0')
        }
    }
}

data class ExtractedTree(
    val rev: Long,
    val window: String,
    val nodes: List<TreeNode>,
    val resolvedViews: List<View>,
    val subtreeNotFound: Boolean = false
)
