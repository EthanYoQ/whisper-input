import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Icon } from '../components/Icon';
import { PreviewButton } from '../components/preview/PreviewPrimitives';
import {
  cancelSelectionPolish,
  confirmSelectionPolish,
  copySelectionPolish,
  isTauri,
} from '../lib/ipc';
import type { SelectionPolishStatePayload } from '../lib/types';

export function SelectionPolishPanel() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<'processing' | 'ready' | 'error'>('processing');
  const [draft, setDraft] = useState('');
  const [sourceApp, setSourceApp] = useState('');
  const [errorCode, setErrorCode] = useState('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    void import('@tauri-apps/api/event').then(async ({ listen }) => {
      unlisten = await listen<SelectionPolishStatePayload>(
        'selection-polish:state',
        event => {
          const payload = event.payload;
          setStatus(payload.kind);
          setSourceApp(payload.sourceApp ?? '');
          setErrorCode(payload.errorCode ?? '');
          if (typeof payload.result === 'string') setDraft(payload.result);
          setBusy(false);
        },
      );
    });
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') void cancelSelectionPolish();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => {
      unlisten?.();
      window.removeEventListener('keydown', onKeyDown);
    };
  }, []);

  const confirm = async () => {
    if (!draft.trim()) return;
    setBusy(true);
    try {
      await confirmSelectionPolish(draft);
    } catch {
      setBusy(false);
    }
  };

  return (
    <div style={shellStyle}>
      <header data-tauri-drag-region style={headerStyle}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <Icon name="sparkle" size={17} />
          <strong>{t('selectionPolish.title')}</strong>
        </div>
        <button
          type="button"
          aria-label={t('common.close')}
          onClick={() => void cancelSelectionPolish()}
          style={iconButtonStyle}
        >
          <Icon name="x" size={16} />
        </button>
      </header>

      <main style={{ padding: 16, display: 'grid', gap: 12 }}>
        {sourceApp && <div style={metaStyle}>{sourceApp}</div>}
        {status === 'processing' ? (
          <div style={stateStyle}>{t('selectionPolish.processing')}</div>
        ) : (
          <textarea
            aria-label={t('selectionPolish.result')}
            value={draft}
            onChange={event => setDraft(event.target.value)}
            style={textareaStyle}
            autoFocus
          />
        )}
        {status === 'error' && (
          <div role="alert" style={errorStyle}>
            {t(`selectionPolish.errors.${errorCode}`, { defaultValue: t('selectionPolish.errorFallback') })}
          </div>
        )}
        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8 }}>
          <PreviewButton style={actionButtonStyle} onClick={() => void cancelSelectionPolish()}>
            {t('common.cancel')}
          </PreviewButton>
          <PreviewButton
            style={actionButtonStyle}
            disabled={!draft.trim() || busy}
            onClick={() => void copySelectionPolish(draft)}
          >
            <Icon name="copy" size={14} />
            {t('common.copy')}
          </PreviewButton>
          <PreviewButton
            style={actionButtonStyle}
            variant="primary"
            disabled={!draft.trim() || busy}
            onClick={() => void confirm()}
          >
            <Icon name="check" size={14} />
            {t('selectionPolish.replace')}
          </PreviewButton>
        </div>
      </main>
    </div>
  );
}

const shellStyle: React.CSSProperties = {
  minHeight: '100vh',
  background: 'rgba(250, 250, 249, 0.96)',
  border: '0.5px solid var(--ol-line)',
  borderRadius: 12,
  color: 'var(--ol-ink-1)',
  fontFamily: 'var(--ol-font-sans)',
  overflow: 'hidden',
};

const headerStyle: React.CSSProperties = {
  height: 48,
  padding: '0 14px',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  borderBottom: '0.5px solid var(--ol-line)',
};

const iconButtonStyle: React.CSSProperties = {
  border: 0,
  background: 'transparent',
  color: 'var(--ol-ink-3)',
  padding: 6,
  minWidth: 40,
  minHeight: 40,
  display: 'grid',
  placeItems: 'center',
};

const actionButtonStyle: React.CSSProperties = { minHeight: 40 };

const textareaStyle: React.CSSProperties = {
  width: '100%',
  minHeight: 210,
  resize: 'vertical',
  boxSizing: 'border-box',
  padding: 12,
  borderRadius: 8,
  border: '0.5px solid var(--ol-line-strong)',
  background: 'var(--ol-surface-1)',
  color: 'var(--ol-ink-1)',
  font: 'inherit',
  lineHeight: 1.6,
};

const metaStyle: React.CSSProperties = { fontSize: 12, color: 'var(--ol-ink-4)' };
const stateStyle: React.CSSProperties = { minHeight: 210, display: 'grid', placeItems: 'center', color: 'var(--ol-ink-3)' };
const errorStyle: React.CSSProperties = { fontSize: 12, color: 'var(--ol-danger)' };
