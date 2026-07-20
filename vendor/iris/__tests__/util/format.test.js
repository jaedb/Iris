import {
  encodeUri,
  decodeUri,
} from '../../src/js/util/format';

const encoded = 'c3BvdGlmeTphbGJ1bTpodHRwOi8vdGVzdC5jb20vMTIzIUAjJCVeJltdOzw+Lz8i4oCU4oCTLcOhw4HDoMOAw6LDgsOkw4TDo8ODw6XDhcOmw4bDp8OHw6nDicOow4jDqsOKw6vDi8Otw43DrMOMw67DjsOvw4/DscORw7PDk8Oyw5LDtMOUw7bDlsO1w5XDuMOYxZPFksOfw7rDmsO5w5nDu8Obw7zDnCAubXAz';
const decoded = 'spotify:album:http://test.com/123!@#$%^&[];<>/?"—–-áÁàÀâÂäÄãÃåÅæÆçÇéÉèÈêÊëËíÍìÌîÎïÏñÑóÓòÒôÔöÖõÕøØœŒßúÚùÙûÛüÜ .mp3';

describe('encodeUri', () => {
  it('should convert uri into base64 string', () => {
    const value = encodeUri(decoded);
    expect(typeof (value)).toBe('string');
    expect(value).toBe(encoded);
  });
});

describe('decodeUri', () => {
  it('should base64 string into original URI', () => {
    const value = decodeUri(encoded);
    expect(typeof (value)).toBe('string');
    expect(value).toBe(decoded);
  });
});
