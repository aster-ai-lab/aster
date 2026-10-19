/** Guest-view shapes the renderer used to import from Electron. The desktop host does not load that package. */

export type WebviewTag = HTMLElement & Record<string, any>

export type DidNavigateEvent = Event & Record<string, any>
export type DidNavigateInPageEvent = Event & Record<string, any>
export type DidStartNavigationEvent = Event & Record<string, any>
export type DidRedirectNavigationEvent = DidStartNavigationEvent
export type DidFailLoadEvent = Event & Record<string, any>
export type PageFaviconUpdatedEvent = Event & Record<string, any>
export type PageTitleUpdatedEvent = Event & Record<string, any>
export type IpcMessageEvent = Event & Record<string, any>
export type ConsoleMessageEvent = Event & Record<string, any>

export interface FoundInPageResult {
  matches: number
  activeMatchOrdinal: number
}

export interface FindInPageOptions {
  forward?: boolean
  findNext?: boolean
}
