export interface SearchAction {
  type: string
  title: string
}

export type ItemType = 'app' | 'file'

export interface SearchItem {
  title: string
  subtitle: string
  file_path: string
  type: ItemType
  actions?: SearchAction[]
  icon_data?: string
  alias?: string
}

export interface SearchResultItem {
  path: string
  file_path: string
  file_name: string
  rank: number
  icon_data?: string
  alias?: string
}

export type UpdateResultPayload = [string, SearchResultItem[], boolean]
