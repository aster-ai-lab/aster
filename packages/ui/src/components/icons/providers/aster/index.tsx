import type { CompoundIcon, CompoundIconProps } from '../../types'
import { AsterAvatar } from './avatar'
import { AsterLight } from './light'

const Aster = ({ variant, className, ...props }: CompoundIconProps) => {
  if (variant === 'light') return <AsterLight {...props} className={className} />
  return <AsterLight {...props} className={className} />
}

export const AsterIcon: CompoundIcon = /*#__PURE__*/ Object.assign(Aster, {
  Avatar: AsterAvatar,
  colorPrimary: '#FF5F5F'
})

export default AsterIcon
