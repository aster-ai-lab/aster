import { mount } from './mount.mjs'

const root = document.getElementById('root')
if (root) mount(root, 'browser')
