import { render } from '@testing-library/react'
import { createElement } from 'react'
import { describe, expect, it } from 'vitest'

import { resetPluginCatalogForTests } from '@renderer/pages/settings/DependenciesSettings/pluginCatalog'

import { annotationOccluders, reviewAnnotationConnector, unionClientRects } from '../annotationConnector'
import { ReviewAnnotationLinks } from '../ReviewAnnotationLinks'
import { publishReviewDocument } from '../reviewFindings'
import { documentScrollTopForQuote } from '../ReviewOfficeDocument'

describe('review annotation links', () => {
  it('draws a curve from the quote leftward onto the finding card', () => {
    const card = new DOMRect(120, 220, 280, 72)
    const mark = new DOMRect(920, 260, 80, 16)
    const link = reviewAnnotationConnector(card, mark)

    expect(link.startX).toBe(920)
    expect(link.startY).toBe(268)
    expect(link.endX).toBe(400)
    expect(link.endY).toBe(242)
    expect(link.d).toBe('M 920 268 C 686 268, 634 242, 400 242')
  })

  it('covers the workspace tab bar and the ribbon with one mask', () => {
    const mask = unionClientRects([new DOMRect(700, 40, 700, 40), new DOMRect(700, 80, 700, 72)])

    expect(mask?.x).toBe(700)
    expect(mask?.y).toBe(40)
    expect(mask?.width).toBe(700)
    expect(mask?.height).toBe(112)
  })

  it('keeps the curve under the settings sheet as well as the workspace header', () => {
    const overlay = document.createElement('div')
    overlay.dataset.ui = 'settings.overlay'
    overlay.getBoundingClientRect = () => new DOMRect(200, 40, 800, 600)
    document.body.append(overlay)
    const holes = annotationOccluders(document)
    overlay.remove()

    expect(holes).toHaveLength(1)
    expect(holes[0]?.x).toBe(200)
    expect(holes[0]?.y).toBe(40)
    expect(holes[0]?.width).toBe(800)
    expect(holes[0]?.height).toBe(600)
  })

  it('keeps the curve under the composer input', () => {
    const composer = document.createElement('div')
    composer.dataset.ui = 'workbench.composer'
    composer.getBoundingClientRect = () => new DOMRect(40, 700, 640, 140)
    const surface = document.createElement('div')
    surface.setAttribute('data-composer-dock-surface', '')
    surface.getBoundingClientRect = () => new DOMRect(80, 720, 560, 110)
    const input = document.createElement('div')
    input.setAttribute('data-composer-inputbar', '')
    input.getBoundingClientRect = () => new DOMRect(96, 748, 528, 72)
    surface.append(input)
    composer.append(surface)
    document.body.append(composer)
    const holes = annotationOccluders(document)
    composer.remove()

    expect(holes.map((hole) => [hole.x, hole.y, hole.width, hole.height])).toEqual([
      [40, 700, 640, 140],
      [80, 720, 560, 110],
      [96, 748, 528, 72]
    ])
  })

  it('centers the document scroller on the quoted line', () => {
    expect(documentScrollTopForQuote(0, 100, 400, 700, 20)).toBe(410)
    expect(documentScrollTopForQuote(20, 100, 400, 120, 20)).toBe(0)
  })

  it('draws no annotation curve while the review surface is unpublished', () => {
    resetPluginCatalogForTests()
    publishReviewDocument({
      path: '/tmp/contract.docx',
      findings: [
        {
          id: 'f1',
          title: '付款期限',
          summary: '期限过短',
          evidenceQuote: '甲方应在三日内付款。',
          guidance: '改为书面通知'
        }
      ]
    })
    const { container } = render(createElement(ReviewAnnotationLinks))
    expect(container.querySelector('[data-review-annotation-links]')).toBeNull()
    expect(document.body.querySelector('[data-review-annotation-links]')).toBeNull()
    expect(reviewAnnotationConnector(new DOMRect(120, 220, 280, 72), new DOMRect(920, 260, 80, 16)).d).toBe(
      'M 920 268 C 686 268, 634 242, 400 242'
    )
    publishReviewDocument(null)
    resetPluginCatalogForTests()
  })
})
