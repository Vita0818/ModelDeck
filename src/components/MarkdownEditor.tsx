import React, { Suspense, lazy } from "react";

export interface MarkdownEditorProps {
  value: string;
  onChange?: (value: string) => void;
  placeholder?: string;
  darkMode?: boolean;
  readOnly?: boolean;
  className?: string;
  minHeight?: string;
  maxHeight?: string;
}

/**
 * CodeMirror is one of the heaviest dependencies in the bundle and is only
 * needed once an editor is actually mounted, so it loads on demand.
 */
const MarkdownEditorView = lazy(() => import("./MarkdownEditor.view"));

const MarkdownEditor: React.FC<MarkdownEditorProps> = (props) => {
  const { className = "", minHeight = "300px" } = props;

  return (
    <Suspense
      fallback={
        <div
          className={`animate-pulse rounded-md border border-border bg-muted ${className}`}
          style={{ minHeight }}
        />
      }
    >
      <MarkdownEditorView {...props} />
    </Suspense>
  );
};

export default MarkdownEditor;
