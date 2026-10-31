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

        traverseView(
            view = rootTarget,
            currentDepth = 0,
            maxDepth = maxDepth,
            density = density,
            collectedNodes = collectedNodes,
            collectedViews = collectedViews
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
        collectedViews: MutableList<View>
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
                editable = isEditable
            )
            collectedNodes.add(treeNode)
            collectedViews.add(view)
        }

        if (view is ViewGroup) {
            for (i in 0 until view.childCount) {
                traverseView(
                    view = view.getChildAt(i),
                    currentDepth = currentDepth + 1,
                    maxDepth = maxDepth,
                    density = density,
                    collectedNodes = collectedNodes,
                    collectedViews = collectedViews
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

    companion object {
        const val DEFAULT_MAX_DEPTH = 12
    }
}

data class ExtractedTree(
    val rev: Long,
    val window: String,
    val nodes: List<TreeNode>,
    val resolvedViews: List<View>,
    val subtreeNotFound: Boolean = false
)
