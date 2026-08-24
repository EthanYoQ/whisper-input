import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Icon } from '../components/Icon';
import {
  activateStylePack, createStylePack, deleteStylePack, duplicateStylePack,
  exportStylePackFile, importStylePackFile,
  listStylePacks, previewStylePack, setStylePackEnabled, updateStylePack,
} from '../lib/ipc';
import type {
  PolishMode, StylePack, StylePackCatalogSnapshot, StylePackDraft, StylePreviewKind,
} from '../lib/types';
import { PreviewButton, PreviewCard, PreviewPageHeader, PreviewPill } from '../components/preview/PreviewPrimitives';

const EMPTY_DRAFT: StylePackDraft = {
  name: '', description: '', baseMode: 'light', dictationPrompt: '', selectionPrompt: '', examples: [],
};
const errorMessage = (error: unknown) => error instanceof Error ? error.message : String(error);
const STYLE_ERROR_CODES = new Set([
  'stylePackReadonlyOrNotFound', 'stylePackNotFound', 'stylePackBuiltinReadonly',
  'stylePackDisabled', 'stylePackExportFailed', 'stylePackImportInvalid',
  'stylePackImportVersion', 'stylePackNameInvalid', 'stylePackContentInvalid',
  'stylePackNameConflict', 'stylePackFileReadFailed', 'stylePackFileWriteFailed',
]);
const safeStyleFileName = (name: string) => `${name.trim().replace(/[<>:"/\\|?*]+/g, '-') || 'style-pack'}.json`;
const draftFrom = (pack: StylePack): StylePackDraft => ({
  name: pack.name, description: pack.description, baseMode: pack.baseMode,
  dictationPrompt: pack.dictationPrompt, selectionPrompt: pack.selectionPrompt, examples: pack.examples,
});

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
  const [catalog, setCatalog] = useState<StylePackCatalogSnapshot | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState<StylePackDraft>(EMPTY_DRAFT);
  const [examplesJson, setExamplesJson] = useState('[]');
  const [previewInput, setPreviewInput] = useState(() => t('stylePacks.previewDefaultInput'));
  const [previewInputTouched, setPreviewInputTouched] = useState(false);
  const [previewKind, setPreviewKind] = useState<StylePreviewKind>('dictation');
  const [previewResult, setPreviewResult] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const active = useMemo(() => catalog?.packs.find(pack => pack.id === catalog.activeStyleId), [catalog]);
  const displayName = (pack: StylePack | undefined) => pack?.kind === 'builtin'
    ? modeLabels[pack.baseMode]
    : pack?.name ?? modeLabels.light;
  const localizedError = (error: unknown) => {
    const message = errorMessage(error);
    if (message === t('stylePacks.examplesJsonInvalid')) return message;
    return STYLE_ERROR_CODES.has(message)
      ? t(`stylePacks.errors.${message}`)
      : t('stylePacks.errors.unknown');
  };

  useEffect(() => {
    let cancelled = false;
    void listStylePacks().then(next => { if (!cancelled) setCatalog(next); })
      .catch(reason => { if (!cancelled) setError(localizedError(reason)); });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (!previewInputTouched) setPreviewInput(t('stylePacks.previewDefaultInput'));
  }, [previewInputTouched, t]);

  async function mutate(operation: () => Promise<StylePackCatalogSnapshot>): Promise<boolean> {
    setBusy(true); setError(null);
    try { setCatalog(await operation()); return true; } catch (reason) { setError(localizedError(reason)); return false; }
    finally { setBusy(false); }
  }

  function resetEditor() {
    setEditingId(null); setDraft(EMPTY_DRAFT); setExamplesJson('[]'); setPreviewResult(''); setError(null);
  }

  function edit(pack: StylePack) {
    if (pack.kind !== 'custom') return;
    setEditingId(pack.id); setDraft(draftFrom(pack));
    setExamplesJson(JSON.stringify(pack.examples, null, 2)); setPreviewResult(''); setError(null);
  }

  function parsedDraft(): StylePackDraft {
    const examples = JSON.parse(examplesJson) as StylePackDraft['examples'];
    if (!Array.isArray(examples)) throw new Error(t('stylePacks.examplesJsonInvalid'));
    return { ...draft, examples };
  }

  async function save() {
    try {
      const value = parsedDraft();
      if (await mutate(() => editingId ? updateStylePack(editingId, value) : createStylePack(value))) resetEditor();
    } catch (reason) { setError(localizedError(reason)); }
  }

  async function preview() {
    setBusy(true); setError(null); setPreviewResult('');
    try { setPreviewResult(await previewStylePack(parsedDraft(), previewInput, previewKind)); }
    catch (reason) { setError(localizedError(reason)); }
    finally { setBusy(false); }
  }

  async function importFromFile() {
    setBusy(true); setError(null);
    try {
      const next = await importStylePackFile();
      if (next) setCatalog(next);
    } catch (reason) { setError(localizedError(reason)); }
    finally { setBusy(false); }
  }

  async function exportToFile(pack: StylePack) {
    setBusy(true); setError(null);
    try { await exportStylePackFile(pack.id, safeStyleFileName(pack.name)); }
    catch (reason) { setError(localizedError(reason)); }
    finally { setBusy(false); }
  }

  if (!catalog) {
    return <div className="wi-style-page"><PreviewPageHeader title={t('style.title')} desc={error ?? t('stylePacks.loading')} /></div>;
  }

  return (
    <div className="wi-style-page">
      <PreviewPageHeader title={t('style.title')} desc={t('stylePacks.currentDescription', { name: displayName(active) })} />
      {error && <div role="alert" className="wi-style-error">{error}</div>}
      <div className="wi-style-actions" style={{ marginBottom: 16 }}>
        <PreviewButton onClick={resetEditor}><Icon name="plus" size={14} /> {t('stylePacks.new')}</PreviewButton>
        <PreviewButton disabled={busy} onClick={() => void importFromFile()}><Icon name="doc" size={14} /> {t('stylePacks.importFile')}</PreviewButton>
      </div>

      <div className="wi-style-grid">
        {catalog.packs.map(pack => {
          const enabled = catalog.enabledStyleIds.includes(pack.id);
          const isActive = catalog.activeStyleId === pack.id;
          return (
            <PreviewCard key={pack.id} className={`wi-style-card ${isActive ? 'wi-style-card-default' : ''} ${enabled ? '' : 'wi-style-card-disabled'}`}>
              <div className="wi-style-card-head"><div>
                <div className="wi-style-name-row"><strong>{displayName(pack)}</strong><PreviewPill>{t(pack.kind === 'builtin' ? 'stylePacks.builtin' : 'stylePacks.local')}</PreviewPill>{isActive && <PreviewPill>{t('stylePacks.active')}</PreviewPill>}</div>
                <div className="wi-style-desc">{pack.kind === 'builtin' ? modeDescriptions[pack.baseMode] : pack.description || modeDescriptions[pack.baseMode]}</div>
              </div></div>
              <div className="wi-style-actions">
                <PreviewButton disabled={busy || !enabled || isActive} onClick={() => void mutate(() => activateStylePack(pack.id))}><Icon name="check" size={14} /> {t('stylePacks.use')}</PreviewButton>
                <PreviewButton disabled={busy} onClick={() => void mutate(() => duplicateStylePack(pack.id))}><Icon name="copy" size={14} /> {t('stylePacks.duplicate')}</PreviewButton>
                {pack.kind === 'custom' && <>
                  <PreviewButton disabled={busy} onClick={() => edit(pack)}><Icon name="settings" size={14} /> {t('stylePacks.edit')}</PreviewButton>
                  <PreviewButton disabled={busy} onClick={() => void mutate(() => setStylePackEnabled(pack.id, !enabled))}>{t(enabled ? 'stylePacks.disable' : 'stylePacks.enable')}</PreviewButton>
                  <PreviewButton disabled={busy} onClick={() => void mutate(() => deleteStylePack(pack.id))}><Icon name="trash" size={14} /> {t('common.delete')}</PreviewButton>
                </>}
                <PreviewButton disabled={busy} onClick={() => void exportToFile(pack)}><Icon name="doc" size={14} /> {t('stylePacks.saveFile')}</PreviewButton>
              </div>
            </PreviewCard>
          );
        })}
      </div>

      <PreviewCard className="wi-style-card" style={{ marginTop: 18 }}>
        <h3>{t(editingId ? 'stylePacks.editCustom' : 'stylePacks.createCustom')}</h3>
        <div style={{ display: 'grid', gap: 10 }}>
          <input className="wi-input" value={draft.name} placeholder={t('stylePacks.name')} onChange={event => setDraft({ ...draft, name: event.target.value })} />
          <input className="wi-input" value={draft.description} placeholder={t('stylePacks.description')} onChange={event => setDraft({ ...draft, description: event.target.value })} />
          <select className="wi-select" value={draft.baseMode} onChange={event => setDraft({ ...draft, baseMode: event.target.value as PolishMode })}>
            {Object.entries(modeLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
          </select>
          <textarea className="wi-input" style={{ minHeight: 90 }} value={draft.dictationPrompt} placeholder={t('stylePacks.dictationPrompt')} onChange={event => setDraft({ ...draft, dictationPrompt: event.target.value })} />
          <textarea className="wi-input" style={{ minHeight: 90 }} value={draft.selectionPrompt} placeholder={t('stylePacks.selectionPrompt')} onChange={event => setDraft({ ...draft, selectionPrompt: event.target.value })} />
          <textarea className="wi-input" style={{ minHeight: 100, fontFamily: 'monospace' }} value={examplesJson} aria-label={t('stylePacks.examplesJson')} onChange={event => setExamplesJson(event.target.value)} />
          <div className="wi-style-actions"><PreviewButton disabled={busy || !draft.name.trim()} onClick={() => void save()}><Icon name="check" size={14} /> {t('stylePacks.save')}</PreviewButton>{editingId && <PreviewButton onClick={resetEditor}><Icon name="x" size={14} /> {t('stylePacks.cancelEdit')}</PreviewButton>}</div>
        </div>

        <h3 style={{ marginTop: 22 }}>{t('stylePacks.previewTitle')}</h3>
        <p className="wi-style-desc">{t('stylePacks.previewDisclosure')}</p>
        <div style={{ display: 'grid', gap: 10 }}>
          <select className="wi-select" value={previewKind} onChange={event => setPreviewKind(event.target.value as StylePreviewKind)}><option value="dictation">{t('stylePacks.previewDictation')}</option><option value="selection">{t('stylePacks.previewSelection')}</option></select>
          <textarea className="wi-input" style={{ minHeight: 80 }} value={previewInput} onChange={event => { setPreviewInputTouched(true); setPreviewInput(event.target.value); }} />
          <PreviewButton disabled={busy || !previewInput.trim()} onClick={() => void preview()}><Icon name="eye" size={14} /> {t('stylePacks.previewAction')}</PreviewButton>
          {previewResult && <textarea className="wi-input" style={{ minHeight: 90 }} value={previewResult} readOnly aria-label={t('stylePacks.previewResult')} />}
        </div>
      </PreviewCard>

    </div>
  );
}
