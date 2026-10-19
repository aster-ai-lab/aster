import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'

import {
  catalogPublishesReview,
  ensurePluginCatalog,
  usePluginCatalog
} from '@renderer/pages/settings/DependenciesSettings/pluginCatalog'

import { annotationOccluders, measureReviewAnnotationLinks, type ReviewAnnotationLink } from './annotationConnector'
import { useActiveReviewFinding, useReviewDocument } from './reviewFindings'

/** Draws the review quote's dashed line leftward, onto the conversation finding card. */
export function ReviewAnnotationLinks() {
  const review = useReviewDocument()
  const activeId = useActiveReviewFinding()
  const catalog = usePluginCatalog()
  const reviewActive = catalogPublishesReview(catalog)

  useEffect(() => {
    void ensurePluginCatalog()
  }, [])
  const [links, setLinks] = useState<ReviewAnnotationLink[]>([])
  const [occluders, setOccluders] = useState<DOMRect[]>([])
  const [viewport, setViewport] = useState({ width: 0, height: 0 })

  useEffect(() => {
    if (!review) {
      setLinks([])
      setOccluders([])
      return
    }
    let frame = 0
    let last = ''
    const measure = () => {
      const next = measureReviewAnnotationLinks(document, activeId)
      const holes = annotationOccluders(document)
      const width = window.innerWidth
      const height = window.innerHeight
      const key = `${width}x${height}|${holes.map((hole) => `${hole.x},${hole.y},${hole.width},${hole.height}`).join(';')}|${next.map((link) => `${link.id}:${link.d}:${link.active}`).join('|')}`
      if (key === last) return
      last = key
      setLinks(next)
      setOccluders(holes)
      setViewport({ width, height })
    }
    const schedule = () => {
      cancelAnimationFrame(frame)
      frame = requestAnimationFrame(measure)
    }
    schedule()
    window.addEventListener('resize', schedule)
    window.addEventListener('scroll', schedule, true)
    const observer = new MutationObserver(schedule)
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['data-comment-id', 'class']
    })
    const timer = window.setInterval(schedule, 400)
    return () => {
      cancelAnimationFrame(frame)
      window.removeEventListener('resize', schedule)
      window.removeEventListener('scroll', schedule, true)
      observer.disconnect()
      window.clearInterval(timer)
    }
  }, [activeId, review])

  if (!review || !reviewActive || links.length === 0) return null
  return createPortal(
    <div className="review-annotation-links" data-review-annotation-links="">
      <svg aria-hidden="true">
        <defs>
          <mask id="review-annotation-below-header" maskUnits="userSpaceOnUse">
            <rect x="0" y="0" width={viewport.width} height={viewport.height} fill="white" />
            {occluders.map((hole) => (
              <rect
                key={`${hole.x},${hole.y},${hole.width},${hole.height}`}
                data-review-annotation-occluder=""
                x={hole.x}
                y={hole.y}
                width={hole.width}
                height={hole.height}
                fill="black"
              />
            ))}
          </mask>
        </defs>
        <g mask="url(#review-annotation-below-header)">
          {links.map((link) => (
            <g key={link.id} data-review-annotation-link={link.id} data-active={link.active ? 'true' : 'false'}>
              <path d={link.d} />
              <circle cx={link.startX} cy={link.startY} r="2.5" />
              <circle cx={link.endX} cy={link.endY} r="2.5" />
            </g>
          ))}
        </g>
      </svg>
    </div>,
    document.body
  )
}
