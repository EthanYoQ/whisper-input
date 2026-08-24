import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Icon } from '../components/Icon';
import {
  activateStylePack,
  createStylePack,
  deleteStylePack,
  duplicateStylePack,
  exportStylePackFile,
  importStylePackFile,
  listStylePacks,
  previewStylePack,
  setStylePackEnabled,
  updateStylePack,
} from '../lib/ipc';
import { resolveStyleDemo, stylePackActionAvailability } from '../lib/stylePresentation';
import type {
  PolishMode,
  StylePack,
  StylePackCatalogSnapshot,
  StylePackDraft,
  StylePreviewKind,
} from '../lib/types';
import {
  PreviewButton,
  PreviewPageHeader,
  PreviewPill,
} from '../components/preview/PreviewPrimitives';
import '../styles/style-page.css';

const EMPTY_DRAFT: StylePackDraft = {
  name: '',
  description: '',
  baseMode: 'light',
  dictationPrompt: '',
  selectionPrompt: '',
  examples: [],
};

const errorMessage = (error: unknown) => error instanceof Error ? error.message : String(error);
const STYLE_ERROR_CODES = new Set([
  'stylePackReadonlyOrNotFound',
  'stylePackNotFound',
  'stylePackBuiltinReadonly',
  'stylePackDisabled',
  'stylePackExportFailed',
  'stylePackImportInvalid',
  'stylePackImportVersion',
  'stylePackNameInvalid',
  'stylePackContentInvalid',
  'stylePackNameConflict',
  'stylePackFileReadFailed',
  'stylePackFileWriteFailed',
]);

const safeStyleFileName = (name: string) => `${name.trim().replace(/[<>:"/\\|?*]+/g, '-') || 'style-pack'}.json`;
const draftFrom = (pack: StylePack): StylePackDraft => ({
  name: pack.name,
  description: pack.description,
  baseMode: pack.baseMode,
  dictationPrompt: pack.dictationPrompt,
  selectionPrompt: pack.selectionPrompt,
  examples: pack.examples,
});

const modeIcon: Record<PolishMode, string> = {
  raw: 'doc',
  light: 'sparkle',
  structured: 'layout',
  formal: 'mail',
};

interface TransientPreview {
  styleId: string;
  input: string;
  output: string;
}

export function Style() {
  const { t } = useTranslation();
  const modeLabels = useMemo<Record<PolishMode, string>>(() => ({
    raw: t('style.modes.raw.name'),
    light: t('style.modes.light.name'),
    structured: t('style.modes.structured.name'),
    formal: t('style.modes.formal.name'),
  }), [t]);
  const modeDescriptions = useMemo<Record<PolishMode, string>>(() => ({
    raw: t('style.modes.raw.desc'),
    light: t('style.modes.light.desc'),
    structured: t('style.modes.structured.desc'),
    formal: t('style.modes.formal.desc'),
  }), [t]);
  const modeSamples = useMemo<Record<PolishMode, string>>(() => ({
    raw: t('style.modes.raw.sample'),
    light: t('style.modes.light.sample'),
    structured: t('style.modes.structured.sample'),
    formal: t('style.modes.formal.sample'),
  }), [t]);

  const [catalog, setCatalog] = useState<StylePackCatalogSnapshot | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedPreviewInput, setSelectedPreviewInput] = useState(() => t('stylePacks.previewDefaultInput'));
  const [transientPreview, setTransientPreview] = useState<TransientPreview | null>(null);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState<StylePackDraft>(EMPTY_DRAFT);
  const [examplesJson, setExamplesJson] = useState('[]');
  const [draftPreviewInput, setDraftPreviewInput] = useState(() => t('stylePacks.previewDefaultInput'));
  const [draftPreviewKind, setDraftPreviewKind] = useState<StylePreviewKind>('dictation');
  const [draftPreviewResult, setDraftPreviewResult] = useState('');
  const [menuOpen, setMenuOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const editorRef = useRef<HTMLElement>(null);
  const editorNameRef = useRef<HTMLInputElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(null);

  const builtinPacks = useMemo(() => catalog?.packs.filter(pack => pack.kind === 'builtin') ?? [], [catalog]);
  const customPacks = useMemo(() => catalog?.packs.filter(pack => pack.kind === 'custom') ?? [], [catalog]);
  const active = useMemo(() => catalog?.packs.find(pack => pack.id === catalog.activeStyleId), [catalog]);
  const selected = useMemo(
    () => catalog?.packs.find(pack => pack.id === selectedId) ?? active ?? catalog?.packs[0],
    [active, catalog, selectedId],
  );

  const displayName = (pack: StylePack | undefined) => pack?.kind === 'builtin'
    ? modeLabels[pack.baseMode]
    : pack?.name ?? modeLabels.light;
  const displayDescription = (pack: StylePack) => pack.kind === 'builtin'
    ? modeDescriptions[pack.baseMode]
    : pack.description || modeDescriptions[pack.baseMode];
  const localizedError = (reason: unknown) => {
    const message = errorMessage(reason);
    if (message === t('stylePacks.examplesJsonInvalid')) return message;
    return STYLE_ERROR_CODES.has(message)
      ? t(`stylePacks.errors.${message}`)
      : t('stylePacks.errors.unknown');
  };

  const selectedTransientPreview = selected && transientPreview?.styleId === selected.id
    ? transientPreview
    : null;
  const selectedPrimaryExampleInput = selected?.kind === 'custom'
    ? selected.examples[0]?.input
    : undefined;
  const selectedDemo = selected
    ? resolveStyleDemo(
      selected,
      modeSamples.raw,
      modeSamples[selected.baseMode],
      selectedTransientPreview,
      selected.kind === 'custom' ? selectedPreviewInput : undefined,
    )
    : null;

  useEffect(() => {
    let cancelled = false;
    void listStylePacks()
      .then(next => {
        if (cancelled) return;
        setCatalog(next);
        setSelectedId(next.activeStyleId);
      })
      .catch(reason => {
        if (!cancelled) setError(localizedError(reason));
      });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (!selected) return;
    const savedInput = selected.kind === 'custom' ? selectedPrimaryExampleInput : modeSamples.raw;
    setSelectedPreviewInput(savedInput || t('stylePacks.previewDefaultInput'));
    setTransientPreview(current => current?.styleId === selected.id ? current : null);
    setMenuOpen(false);
  }, [modeSamples.raw, selected?.id, selectedPrimaryExampleInput, t]);

  useEffect(() => {
    if (!editorOpen) return;
    const frame = window.requestAnimationFrame(() => editorNameRef.current?.focus());
    return () => window.cancelAnimationFrame(frame);
  }, [editorOpen]);

  async function mutate(operation: () => Promise<StylePackCatalogSnapshot>): Promise<StylePackCatalogSnapshot | null> {
    setBusy(true);
    setError(null);
    try {
      const next = await operation();
      setCatalog(next);
      return next;
    } catch (reason) {
      setError(localizedError(reason));
      return null;
    } finally {
      setBusy(false);
    }
  }

  function parsedDraft(): StylePackDraft {
    const examples = JSON.parse(examplesJson) as StylePackDraft['examples'];
    if (!Array.isArray(examples)) throw new Error(t('stylePacks.examplesJsonInvalid'));
    return { ...draft, examples };
  }

  function openNewEditor() {
    returnFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setEditingId(null);
    setDraft(EMPTY_DRAFT);
    setExamplesJson('[]');
    setDraftPreviewInput(t('stylePacks.previewDefaultInput'));
    setDraftPreviewResult('');
    setError(null);
    setEditorOpen(true);
  }

  function openEditor(pack: StylePack) {
    if (pack.kind !== 'custom') return;
    returnFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setEditingId(pack.id);
    setDraft(draftFrom(pack));
    setExamplesJson(JSON.stringify(pack.examples, null, 2));
    setDraftPreviewInput(pack.examples[0]?.input || t('stylePacks.previewDefaultInput'));
    setDraftPreviewResult('');
    setError(null);
    setMenuOpen(false);
    setEditorOpen(true);
  }

  function closeEditor() {
    setEditorOpen(false);
    window.requestAnimationFrame(() => returnFocusRef.current?.focus());
  }

  function handleEditorKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === 'Escape') {
      event.preventDefault();
      closeEditor();
      return;
    }
    if (event.key !== 'Tab') return;
    const focusable = Array.from(editorRef.current?.querySelectorAll<HTMLElement>(
      'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ) ?? []);
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  async function saveDraft() {
    try {
      const value = parsedDraft();
      const next = await mutate(() => editingId
        ? updateStylePack(editingId, value)
        : createStylePack(value));
      if (!next) return;
      closeEditor();
      if (editingId) setSelectedId(editingId);
    } catch (reason) {
      setError(localizedError(reason));
    }
  }

  async function previewDraft() {
    setBusy(true);
    setError(null);
    setDraftPreviewResult('');
    try {
      setDraftPreviewResult(await previewStylePack(parsedDraft(), draftPreviewInput, draftPreviewKind));
    } catch (reason) {
      setError(localizedError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function previewSelected() {
    if (!selected || selected.kind !== 'custom') return;
    setBusy(true);
    setError(null);
    setTransientPreview(null);
    try {
      const output = await previewStylePack(draftFrom(selected), selectedPreviewInput, 'dictation');
      setTransientPreview({ styleId: selected.id, input: selectedPreviewInput, output });
    } catch (reason) {
      setError(localizedError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function saveSelectedPreview() {
    if (!selected || selected.kind !== 'custom' || !selectedTransientPreview?.output.trim()) return;
    const nextExample = {
      title: t('stylePacks.savedExampleTitle'),
      input: selectedTransientPreview.input,
      output: selectedTransientPreview.output,
    };
    const next = await mutate(() => updateStylePack(selected.id, {
      ...draftFrom(selected),
      examples: [nextExample, ...selected.examples.slice(1)],
    }));
    if (next) setTransientPreview(null);
  }

  async function activateSelected() {
    if (!catalog || !selected) return;
    setBusy(true);
    setError(null);
    try {
      setCatalog(await activateStylePack(selected.id));
    } catch (reason) {
      setError(localizedError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function importFromFile() {
    setBusy(true);
    setError(null);
    try {
      const next = await importStylePackFile();
      if (next) setCatalog(next);
    } catch (reason) {
      setError(localizedError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function exportToFile(pack: StylePack) {
    setBusy(true);
    setError(null);
    try {
      await exportStylePackFile(pack.id, safeStyleFileName(displayName(pack)));
    } catch (reason) {
      setError(localizedError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function removeSelected() {
    if (!selected || selected.kind !== 'custom') return;
    const removedId = selected.id;
    const next = await mutate(() => deleteStylePack(removedId));
    if (next) setSelectedId(next.activeStyleId);
    setMenuOpen(false);
  }

  function renderPackRow(pack: StylePack) {
    const isSelected = selected?.id === pack.id;
    const isActive = catalog?.activeStyleId === pack.id;
    const enabled = catalog?.enabledStyleIds.includes(pack.id) ?? false;
    return (
      <button
        key={pack.id}
        type="button"
        className={`wi-style-pack-row ${isSelected ? 'wi-style-pack-row-selected' : ''} ${enabled ? '' : 'wi-style-pack-row-disabled'}`}
        aria-pressed={isSelected}
        onClick={() => setSelectedId(pack.id)}
      >
        <Icon name={pack.kind === 'builtin' ? modeIcon[pack.baseMode] : 'user'} size={20} />
        <span className="wi-style-pack-copy">
          <span className="wi-style-pack-name">
            {displayName(pack)}
            {pack.kind === 'custom' && <span className="wi-style-pack-kind">{t('stylePacks.custom')}</span>}
          </span>
          <span className="wi-style-pack-desc">{displayDescription(pack)}</span>
        </span>
        {isActive && <span className="wi-style-pack-current">{t('stylePacks.active')}</span>}
      </button>
    );
  }

  if (!catalog || !selected || !selectedDemo) {
    return (
      <div className="wi-style-page">
        <PreviewPageHeader title={t('style.title')} desc={error ?? t('stylePacks.loading')} />
      </div>
    );
  }

  const selectedEnabled = catalog.enabledStyleIds.includes(selected.id);
  const selectedActive = catalog.activeStyleId === selected.id;
  const selectedActions = stylePackActionAvailability(selected.kind);

  return (
    <div className="wi-style-page">
      <PreviewPageHeader
        title={t('style.title')}
        desc={t('stylePacks.currentDescription', { name: displayName(active) })}
      />
      {error && <div role="alert" className="wi-style-error wi-style-page-error">{error}</div>}

      <div className="wi-style-workspace">
        <section className="wi-style-browser" aria-label={t('stylePacks.selectorLabel')}>
          <div className="wi-style-browser-actions">
            <PreviewButton onClick={openNewEditor}>
              <Icon name="plus" size={15} /> {t('stylePacks.new')}
            </PreviewButton>
            <PreviewButton disabled={busy} onClick={() => void importFromFile()}>
              <Icon name="doc" size={15} /> {t('stylePacks.importFile')}
            </PreviewButton>
          </div>
          <div className="wi-style-pack-list">
            <div className="wi-style-pack-group">
              <div className="wi-style-pack-group-label">{t('stylePacks.builtinStyles')}</div>
              {builtinPacks.map(renderPackRow)}
            </div>
            <div className="wi-style-pack-group">
              <div className="wi-style-pack-group-label">{t('stylePacks.myStyles')}</div>
              {customPacks.length > 0
                ? customPacks.map(renderPackRow)
                : <div className="wi-style-pack-empty-list">{t('stylePacks.noCustomStyles')}</div>}
            </div>
          </div>
        </section>

        <section className="wi-style-detail" aria-label={t('stylePacks.detailLabel')}>
          <header className="wi-style-detail-head">
            <div className="wi-style-detail-title">
              <Icon name={selected.kind === 'builtin' ? modeIcon[selected.baseMode] : 'user'} size={22} />
              <div>
                <div className="wi-style-detail-name-row">
                  <h2>{displayName(selected)}</h2>
                  <PreviewPill>{t(selected.kind === 'builtin' ? 'stylePacks.builtin' : 'stylePacks.custom')}</PreviewPill>
                  {selectedActive && <PreviewPill tone="blue">{t('stylePacks.active')}</PreviewPill>}
                </div>
                <p>{displayDescription(selected)}</p>
              </div>
            </div>
            <div className="wi-style-detail-actions">
              {selectedActions.canEdit && (
                <PreviewButton onClick={() => openEditor(selected)}>
                  <Icon name="settings" size={15} /> {t('stylePacks.edit')}
                </PreviewButton>
              )}
              <div className="wi-style-menu-wrap">
                <PreviewButton
                  aria-expanded={menuOpen}
                  aria-haspopup="menu"
                  onClick={() => setMenuOpen(open => !open)}
                >
                  <Icon name="settings" size={15} /> {t('stylePacks.manage')}
                </PreviewButton>
                {menuOpen && (
                  <div className="wi-style-menu" role="menu">
                    <button type="button" role="menuitem" disabled={busy} onClick={() => void mutate(() => duplicateStylePack(selected.id))}>
                      <Icon name="copy" size={15} /> {t('stylePacks.duplicate')}
                    </button>
                    <button type="button" role="menuitem" disabled={busy} onClick={() => void exportToFile(selected)}>
                      <Icon name="doc" size={15} /> {t('stylePacks.saveFile')}
                    </button>
                    {selectedActions.canToggle && (
                        <button type="button" role="menuitem" disabled={busy} onClick={() => void mutate(() => setStylePackEnabled(selected.id, !selectedEnabled))}>
                          <Icon name={selectedEnabled ? 'x' : 'check'} size={15} />
                          {t(selectedEnabled ? 'stylePacks.disable' : 'stylePacks.enable')}
                        </button>
                    )}
                    {selectedActions.canDelete && (
                        <button type="button" role="menuitem" className="wi-style-menu-danger" disabled={busy} onClick={() => void removeSelected()}>
                          <Icon name="trash" size={15} /> {t('common.delete')}
                        </button>
                    )}
                  </div>
                )}
              </div>
            </div>
          </header>

          <div className="wi-style-demo-section">
            <div className="wi-style-demo-heading">
              <Icon name="mic" size={17} />
              <h3>{t('stylePacks.inputExample')}</h3>
            </div>
            {selected.kind === 'custom' ? (
              <textarea
                className="wi-style-demo-box wi-style-demo-input"
                value={selectedDemo.source === 'transient' ? selectedDemo.input : selectedPreviewInput}
                onChange={event => {
                  setSelectedPreviewInput(event.target.value);
                  setTransientPreview(null);
                }}
                aria-label={t('stylePacks.inputExample')}
              />
            ) : (
              <div className="wi-style-demo-box wi-style-demo-copy">{selectedDemo.input}</div>
            )}
          </div>

          <div className="wi-style-demo-section wi-style-output-section">
            <div className="wi-style-demo-heading">
              <Icon name="sparkle" size={17} />
              <h3>{t('stylePacks.outputEffect')}</h3>
              {selectedDemo.source === 'saved' && <PreviewPill>{t('stylePacks.savedExample')}</PreviewPill>}
              {selectedDemo.source === 'transient' && <PreviewPill tone="blue">{t('stylePacks.transientPreview')}</PreviewPill>}
            </div>
            {selectedDemo.output ? (
              <div className="wi-style-demo-box wi-style-demo-copy">{selectedDemo.output}</div>
            ) : (
              <div className="wi-style-preview-empty">
                <Icon name="doc" size={30} />
                <strong>{t('stylePacks.previewEmptyTitle')}</strong>
                <PreviewButton
                  variant="primary"
                  disabled={busy || !selectedPreviewInput.trim()}
                  onClick={() => void previewSelected()}
                >
                  <Icon name="sparkle" size={16} /> {t('stylePacks.generatePreview')}
                </PreviewButton>
                <p><Icon name="info" size={14} /> {t('stylePacks.previewPrivacy')}</p>
                <span>{t('stylePacks.previewSaveHint')}</span>
              </div>
            )}
          </div>

          <footer className="wi-style-detail-footer">
            <div className="wi-style-detail-footer-main">
              {!selectedActive && (
                <PreviewButton variant="primary" disabled={busy || !selectedEnabled} onClick={() => void activateSelected()}>
                  <Icon name="check" size={15} /> {t('stylePacks.useStyle')}
                </PreviewButton>
              )}
              {selectedDemo.source === 'transient' && selected.kind === 'custom' && (
                <>
                  <PreviewButton variant="primary" disabled={busy} onClick={() => void saveSelectedPreview()}>
                    <Icon name="check" size={15} /> {t('stylePacks.saveAsExample')}
                  </PreviewButton>
                  <PreviewButton disabled={busy} onClick={() => void previewSelected()}>
                    <Icon name="refresh" size={15} /> {t('stylePacks.regeneratePreview')}
                  </PreviewButton>
                </>
              )}
            </div>
            {!selectedEnabled && <span className="wi-style-disabled-note">{t('stylePacks.disabledNote')}</span>}
          </footer>
        </section>
      </div>

      {editorOpen && (
        <div className="wi-style-editor-backdrop" role="presentation" onMouseDown={closeEditor}>
          <section
            ref={editorRef}
            className="wi-style-editor"
            role="dialog"
            aria-modal="true"
            aria-labelledby="style-editor-title"
            onMouseDown={event => event.stopPropagation()}
            onKeyDown={handleEditorKeyDown}
          >
            <header className="wi-style-editor-head">
              <div>
                <h2 id="style-editor-title">{t(editingId ? 'stylePacks.editCustom' : 'stylePacks.createCustom')}</h2>
                <p>{t('stylePacks.editorDescription')}</p>
              </div>
              <button type="button" className="wi-style-editor-close" aria-label={t('common.close')} onClick={closeEditor}>
                <Icon name="x" size={18} />
              </button>
            </header>
            <div className="wi-style-editor-body">
              <div className="wi-style-editor-fields">
                <label>
                  <span>{t('stylePacks.name')}</span>
                  <input ref={editorNameRef} className="wi-input" value={draft.name} onChange={event => setDraft({ ...draft, name: event.target.value })} />
                </label>
                <label>
                  <span>{t('stylePacks.description')}</span>
                  <input className="wi-input" value={draft.description} onChange={event => setDraft({ ...draft, description: event.target.value })} />
                </label>
                <label>
                  <span>{t('stylePacks.baseMode')}</span>
                  <select className="wi-select" value={draft.baseMode} onChange={event => setDraft({ ...draft, baseMode: event.target.value as PolishMode })}>
                    {Object.entries(modeLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
                  </select>
                </label>
                <label>
                  <span>{t('stylePacks.dictationPrompt')}</span>
                  <textarea className="wi-input" value={draft.dictationPrompt} onChange={event => setDraft({ ...draft, dictationPrompt: event.target.value })} />
                </label>
                <label>
                  <span>{t('stylePacks.selectionPrompt')}</span>
                  <textarea className="wi-input" value={draft.selectionPrompt} onChange={event => setDraft({ ...draft, selectionPrompt: event.target.value })} />
                </label>
                <label>
                  <span>{t('stylePacks.examplesJson')}</span>
                  <textarea className="wi-input wi-style-examples-json" value={examplesJson} onChange={event => setExamplesJson(event.target.value)} />
                </label>
              </div>
              <div className="wi-style-draft-preview">
                <h3>{t('stylePacks.previewTitle')}</h3>
                <p>{t('stylePacks.previewDisclosure')}</p>
                <select className="wi-select" value={draftPreviewKind} onChange={event => setDraftPreviewKind(event.target.value as StylePreviewKind)}>
                  <option value="dictation">{t('stylePacks.previewDictation')}</option>
                  <option value="selection">{t('stylePacks.previewSelection')}</option>
                </select>
                <textarea className="wi-input" value={draftPreviewInput} onChange={event => setDraftPreviewInput(event.target.value)} />
                <PreviewButton disabled={busy || !draftPreviewInput.trim()} onClick={() => void previewDraft()}>
                  <Icon name="eye" size={15} /> {t('stylePacks.previewAction')}
                </PreviewButton>
                {draftPreviewResult && <div className="wi-style-draft-preview-result">{draftPreviewResult}</div>}
              </div>
            </div>
            <footer className="wi-style-editor-footer">
              <PreviewButton onClick={closeEditor}>{t('stylePacks.cancelEdit')}</PreviewButton>
              <PreviewButton variant="primary" disabled={busy || !draft.name.trim()} onClick={() => void saveDraft()}>
                <Icon name="check" size={15} /> {t('stylePacks.save')}
              </PreviewButton>
            </footer>
          </section>
        </div>
      )}
    </div>
  );
}
