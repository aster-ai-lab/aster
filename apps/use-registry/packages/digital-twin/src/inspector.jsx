import { Button } from '@aster-ai-lab/ui/components/primitives/button'
import { Input } from '@aster-ai-lab/ui/components/primitives/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@aster-ai-lab/ui/components/primitives/select'
import { useEffect, useRef, useState } from 'react'

import * as THREE from '../../g1-arm/ui/workbench/vendor/three/three.module.min.js'

function draftFor(entity) {
  const euler = new THREE.Euler().setFromQuaternion(new THREE.Quaternion().fromArray(entity.transform.rotation), 'XYZ')
  return {
    name: entity.name,
    position: entity.transform.position.map(String),
    rotation: [euler.x, euler.y, euler.z].map((value) => String(Number(THREE.MathUtils.radToDeg(value).toFixed(6)))),
    scale: entity.transform.scale.map(String),
    motion: entity.body?.motion ?? 'static',
    mass: String(entity.body?.mass ?? 0),
    friction: String(entity.body?.friction ?? 0)
  }
}

export function Inspector({ id, entity, revision, onEdit, register, t, disabled = false, initialDraft, onDraftChange }) {
  const restored = initialDraft?.id === id ? initialDraft : null
  const [draft, setDraft] = useState(() => restored?.draft ?? draftFor(entity))
  const [reviewed, setReviewed] = useState(restored?.reviewed ?? revision)
  const [dirty, setDirty] = useState(!!restored)
  const [error, setError] = useState(null)
  const [fields, setFields] = useState([])
  const summary = useRef(null)
  const reset = () => {
    setDraft(draftFor(entity))
    setReviewed(revision)
    setDirty(false)
    setError(null)
    setFields([])
    onDraftChange?.(null)
  }
  useEffect(() => {
    if (!dirty) reset()
  }, [entity, revision])
  const commit = () => {
    if (disabled) return false
    if (!dirty) return true
    const vectors = Object.fromEntries(['position', 'rotation', 'scale'].map((key) => [key, draft[key].map(Number)]))
    const invalid = []
    if (!draft.name.trim() || [...draft.name].length > 128) invalid.push({ id: 'entity-name', label: t('name') })
    for (const key of ['position', 'rotation', 'scale'])
      for (const [index, axis] of ['x', 'y', 'z'].entries()) {
        if (
          !draft[key][index].trim() ||
          !Number.isFinite(vectors[key][index]) ||
          (key === 'scale' && vectors[key][index] <= 0)
        )
          invalid.push({ id: `${key}-${axis}`, label: t('axis_field', { property: t(key), axis: axis.toUpperCase() }) })
      }
    if (entity.body)
      for (const key of ['mass', 'friction']) {
        if (
          !draft[key].trim() ||
          !Number.isFinite(Number(draft[key])) ||
          Number(draft[key]) < 0 ||
          (key === 'mass' && draft.motion !== 'static' && Number(draft[key]) <= 0)
        )
          invalid.push({ id: `body-${key}`, label: t(key) })
      }
    setFields(invalid)
    if (invalid.length) {
      setError('invalid_properties')
      queueMicrotask(() => summary.current?.focus())
      return false
    }
    const next = structuredClone(entity)
    next.name = draft.name
    next.transform = {
      position: vectors.position,
      scale: vectors.scale,
      rotation: new THREE.Quaternion()
        .setFromEuler(new THREE.Euler(...vectors.rotation.map(THREE.MathUtils.degToRad), 'XYZ'))
        .toArray()
    }
    if (next.body)
      Object.assign(next.body, {
        motion: draft.motion,
        mass: Number(draft.mass),
        friction: Number(draft.friction)
      })
    try {
      onEdit([{ kind: 'replace', id, entity: next }], reviewed)
      setDirty(false)
      setError(null)
      onDraftChange?.(null)
      return true
    } catch (failure) {
      setError(String(failure).startsWith('twin.revision_conflict:') ? 'conflict' : 'invalid_properties')
      queueMicrotask(() => summary.current?.focus())
      return false
    }
  }
  useEffect(() => {
    register.current = commit
    return () => {
      register.current = () => true
    }
  })
  const change = (key, value) => {
    const next = { ...draft, [key]: value }
    setDraft(next)
    setDirty(true)
    onDraftChange?.({ id, reviewed, draft: next })
  }
  // Studio's sandbox disables HTML form submission, including the submit event.
  return (
    <form
      className="twin-properties"
      onSubmit={(event) => event.preventDefault()}
      onKeyDown={(event) => {
        if (event.key === 'Enter' && !event.nativeEvent.isComposing && event.target.tagName === 'INPUT') {
          event.preventDefault()
          commit()
        }
      }}>
      <h2>{t('properties')}</h2>
      <p className="twin-entity-id">{id}</p>
      {error && (
        <div id="properties-error" ref={summary} tabIndex={-1} role="alert" className="twin-error">
          <p>{t(error)}</p>
          {fields.length > 0 && (
            <ul>
              {fields.map((field) => (
                <li key={field.id}>
                  <a
                    href={`#${field.id}`}
                    onClick={(event) => {
                      event.preventDefault()
                      document.getElementById(field.id)?.focus()
                    }}>
                    {field.label}
                  </a>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
      <label htmlFor="entity-name">{t('name')}</label>
      <Input
        id="entity-name"
        disabled={disabled}
        required
        aria-invalid={fields.some((field) => field.id === 'entity-name') || undefined}
        aria-describedby={error ? 'properties-error' : undefined}
        value={draft.name}
        onChange={(event) => change('name', event.target.value)}
      />
      {['position', 'rotation', 'scale'].map((key) => (
        <fieldset key={key}>
          <legend>{t(key)}</legend>
          <div className="twin-vector">
            {['x', 'y', 'z'].map((axis, index) => (
              <label key={axis}>
                <span>{axis.toUpperCase()}</span>
                <Input
                  id={`${key}-${axis}`}
                  disabled={disabled}
                  inputMode="decimal"
                  aria-invalid={fields.some((field) => field.id === `${key}-${axis}`) || undefined}
                  aria-describedby={error ? 'properties-error' : undefined}
                  aria-label={t('axis_field', { property: t(key), axis: axis.toUpperCase() })}
                  value={draft[key][index]}
                  onChange={(event) => {
                    const values = [...draft[key]]
                    values[index] = event.target.value
                    change(key, values)
                  }}
                />
              </label>
            ))}
          </div>
        </fieldset>
      ))}
      {entity.body && (
        <fieldset>
          <legend>{t('physics')}</legend>
          <label htmlFor="body-motion">{t('motion')}</label>
          <Select value={draft.motion} disabled={disabled} onValueChange={(value) => change('motion', value)}>
            <SelectTrigger id="body-motion">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {['static', 'dynamic', 'kinematic'].map((value) => (
                <SelectItem key={value} value={value}>
                  {t(value)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <label htmlFor="body-mass">{t('mass')}</label>
          <Input
            id="body-mass"
            disabled={disabled}
            aria-invalid={fields.some((field) => field.id === 'body-mass') || undefined}
            aria-describedby={error ? 'properties-error' : undefined}
            inputMode="decimal"
            value={draft.mass}
            onChange={(event) => change('mass', event.target.value)}
          />
          <label htmlFor="body-friction">{t('friction')}</label>
          <Input
            id="body-friction"
            disabled={disabled}
            aria-invalid={fields.some((field) => field.id === 'body-friction') || undefined}
            aria-describedby={error ? 'properties-error' : undefined}
            inputMode="decimal"
            value={draft.friction}
            onChange={(event) => change('friction', event.target.value)}
          />
        </fieldset>
      )}
      <div className="twin-form-actions">
        <Button variant="outline" type="button" disabled={disabled || (!dirty && !error)} onClick={reset}>
          {t('reset_properties')}
        </Button>
        <Button type="button" disabled={disabled || !dirty} onClick={commit}>
          {t('apply')}
        </Button>
      </div>
    </form>
  )
}
