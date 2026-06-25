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

export type SearchIndexState =
  | 'unbuilt'
  | 'building'
  | 'released'
  | 'loading'
  | 'ready'
  | 'error'
  | 'unavailable'

export interface VolumeIndexStatus {
  name: string
  indexed: boolean
  indexItemCount?: number | null
  indexFileSizeBytes: number
  indexFileModifiedAt?: number | null
}

export interface SearchIndexStatus {
  state: SearchIndexState
  volumeCount: number
  indexedVolumeCount: number
  indexItemCount: number
  indexFileSizeBytes: number
  latestIndexModifiedAt?: number | null
  volumes: VolumeIndexStatus[]
}
