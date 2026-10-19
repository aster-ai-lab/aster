import type { CompoundIcon, CompoundIconProps } from '../../types'
import { A3sAvatar } from './avatar'
import { A3sLight } from './light'

const A3s = ({ variant, className, ...props }: CompoundIconProps) => {
  if (variant === 'light') return <A3sLight {...props} className={className} />
  return <A3sLight {...props} className={className} />
}

export const A3sIcon: CompoundIcon = /*#__PURE__*/ Object.assign(A3s, {
  Avatar: A3sAvatar,
  colorPrimary: '#FF5F5F'
})

export default A3sIcon
