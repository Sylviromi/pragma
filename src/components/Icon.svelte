<script lang="ts">
  // Material Symbols Rounded (font-based) icon renderer.
  // Names map to the ASCII ligatures of the "Material Symbols Rounded" font
  // (bundled via the `material-symbols` npm package) — same look as the
  // design's icon usage, with `FILL 0 / wght 300 / GRAD 0 / opsz 24` applied
  // through the shared `.msr` class in tokens.css.
  const ICONS = {
    // ＋ — new session
    add: 'add',
    // ⌄ — dropdown affordance
    expand_more: 'expand_more',
    // › — collapsed toggle
    chevron_right: 'chevron_right',
    // ‹ — back
    chevron_left: 'chevron_left',
    // ✓ — success / selected
    check: 'check',
    // ⟳ — running spinner
    sync: 'sync',
    // ⚡ — waiting / pending
    bolt: 'bolt',
    // gear — settings
    settings: 'settings',
    // puzzle — plugins
    extension: 'extension',
    // × — close
    close: 'close',
    // ■ — stop
    stop: 'stop',
    // camera — screenshot
    camera: 'camera',
    // download — install
    download: 'download',
    // folder — browse
    folder: 'folder_open',
    // assistant avatar spark
    sparkle: 'auto_awesome',
    // thought block (design)
    neurology: 'neurology',
    // question block (design)
    help: 'help',
    smart_toy: 'smart_toy',
    // @ skills (design)
    alternate_email: 'alternate_email',
    // / commands (design)
    terminal: 'terminal',
    // send (design)
    arrow_upward: 'arrow_upward',
    // copy code (design)
    content_copy: 'content_copy',
    // sidebar search / tool grep
    search: 'search',
    // sidebar sections (design)
    checklist: 'checklist',
    folder_open: 'folder_open',
    difference: 'difference',
    // tool status (design)
    progress_activity: 'progress_activity',
    // plan items (design)
    task_alt: 'task_alt',
    radio_button_checked: 'radio_button_checked',
    radio_button_unchecked: 'radio_button_unchecked',
    // tool call icons (design)
    description: 'description',
    edit_document: 'edit_document',
    note_add: 'note_add',
    travel_explore: 'travel_explore',
    // sidebar toggle (design)
    right_panel_open: 'right_panel_open',
    // ── added during componentization audit (one registry for the whole app) ──
    account_tree: 'account_tree',
    arrow_back: 'arrow_back',
    auto_awesome: 'auto_awesome',
    build: 'build',
    contact_support: 'contact_support',
    deployed_code_update: 'deployed_code_update',
    drive_folder_upload: 'drive_folder_upload',
    edit: 'edit',
    error: 'error',
    fact_check: 'fact_check',
    fork_right: 'fork_right',
    front_hand: 'front_hand',
    hub: 'hub',
    info: 'info',
    key: 'key',
    keyboard: 'keyboard',
    left_panel_open: 'left_panel_open',
    open_in_new: 'open_in_new',
    palette: 'palette',
    pause: 'pause',
    rate_review: 'rate_review',
    search_off: 'search_off',
    stop_circle: 'stop_circle',
    storefront: 'storefront',
    tune: 'tune',
    unfold_more: 'unfold_more',
    verified: 'verified',
    vertical_align_bottom: 'vertical_align_bottom',
    warning: 'warning',
  } as const

  let {
    name,
    size = 18,
    fill = false,
  }: {
    name: keyof typeof ICONS
    size?: number
    /** Filled icon variant (FILL axis = 1). */
    fill?: boolean
  } = $props()
</script>

<span
  class="msr"
  class:fill
  style:font-size={`${size}px`}
  style:font-variation-settings={fill ? "'FILL' 1, 'wght' 400, 'GRAD' 0, 'opsz' 24" : undefined}
  aria-hidden="true"
  >{ICONS[name]}</span
>

<style>
  .fill {
    font-variation-settings: 'FILL' 1, 'wght' 400, 'GRAD' 0, 'opsz' 24;
  }
</style>