import React from 'react';
import TestRenderer from 'react-test-renderer';
import { Provider as ReduxProvider } from 'react-redux/src';
import { BrowserRouter, Route, Routes } from 'react-router-dom';
import { DndProvider } from 'react-dnd';
import { HTML5Backend } from 'react-dnd-html5-backend';
import { buildStore } from '../src/js/store';

const customRender = (
  element,
  {
    initialState,
    mocks,
    ...options
  } = {},
) => {
  return TestRenderer.create(
    (
      <ReduxProvider store={buildStore({ initialState })}>
        <DndProvider backend={HTML5Backend}>
          <BrowserRouter basename="/">
            <Routes>
              <Route path="*" element={element} />
            </Routes>
          </BrowserRouter>
        </DndProvider>
      </ReduxProvider>
    ),
    {
      ...options,
    },
  );
}

export { customRender as render };
