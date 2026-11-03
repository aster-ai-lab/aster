import * as SliderPrimitive from '@radix-ui/react-slider'
import { cva } from 'class-variance-authority'
import * as React from 'react'

import { cn } from '../../lib/utils'

import arcSlider from '../arc/slider/slider.module.css'

type SliderMark = {
  value: number
  label: React.ReactNode
}

const sliderMarkLabelVariants = cva('absolute top-0 leading-none whitespace-nowrap text-muted-foreground', {
  variants: {
    size: {
      sm: 'text-[10px]',
      md: 'text-xs',
      lg: 'text-sm'
    }
  },
  defaultVariants: {
    size: 'md'
  }
})

function Slider({
  className,
  defaultValue,
  value,
  min = 0,
  max = 100,
  size,
  marks,
  orientation = 'horizontal',
  showValueLabel,
  formatValueLabel,
  getThumbAriaLabel,
  getThumbAriaValueText,
  onValueChange,
  ...props
}: React.ComponentProps<typeof SliderPrimitive.Root> & {
  size?: 'sm' | 'md' | 'lg'
  marks?: SliderMark[]
  showValueLabel?: boolean
  formatValueLabel?: (value: number) => React.ReactNode
  getThumbAriaLabel?: (index: number) => string
  getThumbAriaValueText?: (value: number, index: number) => string
}) {
  const [localValues, setLocalValues] = React.useState(() =>
    Array.isArray(value) ? value : Array.isArray(defaultValue) ? defaultValue : [min]
  )

  React.useEffect(() => {
    if (Array.isArray(value)) {
      setLocalValues(value)
    }
  }, [value])

  const isVertical = orientation === 'vertical'

  const sliderElement = (
    <SliderPrimitive.Root
      data-slot="slider"
      data-size={size}
      defaultValue={defaultValue}
      value={value}
      min={min}
      max={max}
      orientation={orientation}
      onValueChange={(newValues) => {
        setLocalValues(newValues)
        onValueChange?.(newValues)
      }}
      className={cn(
        'relative flex w-full touch-none items-center select-none data-[disabled]:opacity-40 data-[orientation=vertical]:h-full data-[orientation=vertical]:min-h-44 data-[orientation=vertical]:w-auto data-[orientation=vertical]:flex-col',
        !marks?.length && className
      )}
      {...props}>
      <SliderPrimitive.Track data-slot="slider-track" data-size={size ?? 'md'} className={cn('arc', arcSlider.track)}>
        <SliderPrimitive.Range
          data-slot="slider-range"
          className={cn(
            'arc',
            arcSlider.fill,
            'data-[orientation=horizontal]:h-full data-[orientation=vertical]:w-full'
          )}
        />
      </SliderPrimitive.Track>
      {localValues.map((val, index) => (
        <SliderPrimitive.Thumb
          data-slot="slider-thumb"
          data-size={size ?? 'md'}
          key={index}
          aria-label={getThumbAriaLabel?.(index)}
          aria-valuetext={getThumbAriaValueText?.(val, index)}
          className={cn(
            'arc',
            arcSlider.thumb,
            'shrink-0 disabled:pointer-events-none disabled:opacity-40',
            showValueLabel && 'group'
          )}>
          {showValueLabel && (
            <span
              data-slot="slider-value-label"
              className={cn(
                'arc',
                arcSlider.bubble,
                'pointer-events-none absolute left-1/2 -translate-x-1/2 -translate-y-full',
                size === 'sm' ? '-top-1 text-[10px]' : size === 'lg' ? '-top-2 text-sm' : '-top-1.5 text-xs',
                'opacity-0 motion-safe:scale-0 motion-safe:transition-[scale,opacity] motion-safe:duration-150 group-hover:scale-100 group-hover:opacity-100 group-focus-within:scale-100 group-focus-within:opacity-100'
              )}>
              {formatValueLabel ? formatValueLabel(val) : val}
            </span>
          )}
        </SliderPrimitive.Thumb>
      ))}
    </SliderPrimitive.Root>
  )

  if (!marks?.length) {
    return sliderElement
  }

  return (
    <div
      data-slot="slider-container"
      className={cn('relative', isVertical ? 'flex h-full items-stretch' : '', className)}>
      {sliderElement}
      <div
        data-slot="slider-marks"
        className={cn('relative', isVertical ? 'ml-2 flex h-full flex-col justify-between' : 'mt-1.5 h-4 w-full')}>
        {marks.map((mark) => {
          const range = max - min
          if (range === 0) return null
          const percentage = ((mark.value - min) / range) * 100
          const transform =
            percentage <= 0 ? 'translateX(0)' : percentage >= 100 ? 'translateX(-100%)' : 'translateX(-50%)'
          return (
            <span
              key={mark.value}
              data-slot="slider-mark"
              className={sliderMarkLabelVariants({ size })}
              style={
                isVertical
                  ? { top: `${100 - percentage}%`, transform: 'translateY(-50%)' }
                  : { left: `${percentage}%`, transform }
              }>
              {mark.label}
            </span>
          )
        })}
      </div>
    </div>
  )
}

export { Slider, type SliderMark }
