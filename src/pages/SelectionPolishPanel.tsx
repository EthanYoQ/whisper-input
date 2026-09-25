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
import { asyncSubscription } from '../lib/asyncSubscription';
import { applySelectionPolishEvent, canCopySelectionPolish, initialSelectionPolishPreviewState } from '../lib/selectionPolishState';

export function SelectionPolishPanel() {
  const { t } = useTranslation();
  const [preview, setPreview] = useState(initialSelectionPolishPreviewState);
  const { status, draft, sourceApp, errorCode, busy, requestId } = preview;

  useEffect(() => {
    if (!isTauri) return;
    const stop = asyncSubscription(async () => {
      const { listen } = await import('@tauri-apps/api/event');
      return listen<SelectionPolishStatePayload>(
        'selection-polish:state',
        event => {
          setPreview(current => applySelectionPolishEvent(current, event.payload));
        },
      );
    }, error => {
      console.warn('[selection-polish] listener failed', error);
      setPreview(current => ({ ...current, status: 'error', errorCode: 'listenerUnavailable', busy: false }));
    });
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setPreview(current => ({ ...current, requestId: null }));
        void cancelSelectionPolish();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => {
      stop();
      window.removeEventListener('keydown', onKeyDown);
    };
  }, []);

  const confirm = async () => {
    if (status !== 'ready' || !requestId || !draft.trim() || busy) return;
    setPreview(current => ({ ...current, busy: true }));
    try {
      await confirmSelectionPolish(requestId, draft);
    } catch {
      setPreview(current => ({ ...current, busy: false }));
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
          onClick={() => { setPreview(current => ({ ...current, requestId: null })); void cancelSelectionPolish(); }}
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
            onChange={event => setPreview(current => ({ ...current, draft: event.target.value }))}
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
          <PreviewButton style={actionButtonStyle} onClick={() => { setPreview(current => ({ ...current, requestId: null })); void cancelSelectionPolish(); }}>
            {t('common.cancel')}
          </PreviewButton>
          <PreviewButton
            style={actionButtonStyle}
            disabled={!canCopySelectionPolish(preview)}
            onClick={() => void copySelectionPolish(draft)}
          >
            <Icon name="copy" size={14} />
            {t('common.copy')}
          </PreviewButton>
          <PreviewButton
            style={actionButtonStyle}
            variant="primary"
            disabled={status !== 'ready' || !requestId || !draft.trim() || busy}
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
  width: '100%',
  height: '100vh',
  background: 'linear-gradient(180deg, var(--lg-float-top), var(--lg-float-bottom))',
  border: '0.5px solid var(--lg-float-border)',
  borderRadius: 12,
  color: 'var(--ol-ink)',
  boxShadow: 'var(--lg-float-shadow)',
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
  background: 'transparent',
  color: 'var(--ol-ink)',
  font: 'inherit',
  lineHeight: 1.6,
};

const metaStyle: React.CSSProperties = { fontSize: 12, color: 'var(--ol-ink-4)' };
const stateStyle: React.CSSProperties = { minHeight: 210, display: 'grid', placeItems: 'center', color: 'var(--ol-ink-3)' };
const errorStyle: React.CSSProperties = { fontSize: 12, color: 'var(--ol-err)' };
