// Shared tool-name → Material glyph mapping. Previously duplicated between
// ToolCard.iconFor() and SubagentCard.TOOL_ICONS (audit D5). Tool families
// map to their design glyphs; anything else falls back to a neutral build icon.
export function toolIcon(name: string): string {
  switch (true) {
    case name.startsWith('read'): return 'description'
    case name.startsWith('write'): return 'note_add'
    case name.startsWith('patch') || name.startsWith('edit'): return 'edit_document'
    case name.startsWith('create') || name.startsWith('delete') || name.startsWith('move'): return 'note_add'
    case name.startsWith('run'): return 'terminal'
    case name.startsWith('list') || name.startsWith('search') || name.startsWith('grep'): return 'travel_explore'
    case name.startsWith('git'): return 'difference'
    case name.startsWith('web') || name.startsWith('fetch'): return 'travel_explore'
    default: return 'build'
  }
}
