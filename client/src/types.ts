export interface Folder {
  id: number
  parent_id: number | null
  name: string
  code: string
  stage: string
  status: string
  description: string
  remarks: string
  creator: string
  created_at: string
}

export interface FileMeta {
  id: number
  folder_id: number | null
  name: string
  ext: string
  size: number
  description: string
  code: string
  stage: string
  status: string
  remarks: string
  creator: string
  drawing_size: string
  source_file_type: string
  source_file_version: string
  other_info: string
  publish_time: string
  current_version: number
  created_at: string
  updated_at: string
}

export interface VersionInfo {
  id: number
  file_id: number
  version_no: number
  size: number
  sha256: string
  comment: string
  created_at: string
}

export interface Tree {
  folders: Folder[]
  files: FileMeta[]
}
