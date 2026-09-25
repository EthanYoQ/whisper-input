import type { SelectionPolishStatePayload } from './types';

export interface SelectionPolishPreviewState {
  requestId: string | null;
  status: 'processing' | 'ready' | 'error';
  draft: string;
  sourceApp: string;
  errorCode: string;
  busy: boolean;
}

export const initialSelectionPolishPreviewState: SelectionPolishPreviewState = {
  requestId: null,
  status: 'processing',
  draft: '',
  sourceApp: '',
  errorCode: '',
  busy: false,
};

export function applySelectionPolishEvent(
  state: SelectionPolishPreviewState,
  payload: SelectionPolishStatePayload,
): SelectionPolishPreviewState {
  if (payload.kind === 'processing' || (payload.kind === 'error' && !payload.requestId)) {
    return {
      requestId: payload.requestId ?? null,
      status: payload.kind,
      draft: '',
      sourceApp: payload.sourceApp ?? '',
      errorCode: payload.errorCode ?? '',
      busy: false,
    };
  }
  if (!payload.requestId || payload.requestId !== state.requestId) return state;
  return {
    ...state,
    status: payload.kind,
    draft: payload.kind === 'ready' && typeof payload.result === 'string' ? payload.result : state.draft,
    sourceApp: payload.sourceApp ?? '',
    errorCode: payload.errorCode ?? '',
    busy: false,
  };
}
