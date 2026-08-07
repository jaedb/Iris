import { useState, useEffect } from 'react';

const OPTIONS = {
  root: null,
  rootMargin: '0px 0px 0px 0px',
  threshold: 0,
};

const useVisible = (elementRef) => {
  const [isVisible, setIsVisible] = useState(false);

  useEffect(() => {
    if (elementRef.current) {
      const observer = new IntersectionObserver((entries, observer2) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            setIsVisible(true);
            observer2.unobserve(elementRef.current);
          }
        });
      }, OPTIONS);
      observer.observe(elementRef.current);
    }
  }, [elementRef]);

  return isVisible;
};

export default useVisible;
