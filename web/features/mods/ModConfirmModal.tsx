import { Check, TriangleAlert, X } from "lucide-react";
import type { ModConfirmAnswer, ModConfirmDialogState } from "../../hooks/useModActions";

function ModConfirmModal(props: {
  dialog: ModConfirmDialogState;
  onAnswer: (value: ModConfirmAnswer) => void;
}) {
  return (
    <div className="modal-backdrop">
      <section className="modal mod-confirm-modal" role="dialog" aria-modal="true" aria-label={props.dialog.title}>
        <header>
          <h2>{props.dialog.title}</h2>
          {props.dialog.body.map((line) => (
            <p key={line}>{line}</p>
          ))}
        </header>
        {props.dialog.items.length > 0 && (
          <div className="mod-confirm-list">
            {props.dialog.items.map((item) => (
              <span key={item}>
                <TriangleAlert size={14} />
                {item}
              </span>
            ))}
          </div>
        )}
        <footer>
          <button type="button" className="icon-button-text compact" onClick={() => props.onAnswer("cancel")}>
            <X size={15} />
            {props.dialog.cancelLabel}
          </button>
          {props.dialog.secondaryLabel && (
            <button type="button" className="icon-button-text compact" onClick={() => props.onAnswer("secondary")}>
              {props.dialog.secondaryLabel}
            </button>
          )}
          <button type="button" className="primary icon-button-text compact" onClick={() => props.onAnswer("confirm")}>
            <Check size={15} />
            {props.dialog.confirmLabel}
          </button>
        </footer>
      </section>
    </div>
  );
}

export { ModConfirmModal };
