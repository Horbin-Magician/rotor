export interface TranslateResult {
  text: string
  translated: string
  from: string
  to: string
}

export type TranslateStreamEvent =
  | {
      event: 'started'
      text: string
      from: string
      to: string
    }
  | {
      event: 'delta'
      content: string
    }
