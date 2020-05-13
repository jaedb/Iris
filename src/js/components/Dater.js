
import { memo } from 'react';

/**
 * Format time duration
 *
 * @param milliseconds int
 * @return string (HH:mm:ss)
 * */
const durationTime = (milliseconds = null) => {
  if (!milliseconds) return null;

  let string = '';
  let total_hours; let total_minutes; let total_seconds; let minutes; let
    seconds;

  // get total values for each
  total_seconds = Math.floor(milliseconds / 1000);
  total_minutes = Math.floor(milliseconds / (1000 * 60));
  total_hours = Math.floor(milliseconds / (1000 * 60 * 60));

  // get left-over number of seconds
  seconds = total_seconds - (total_minutes * 60);
  if (seconds <= 9) seconds = `0${seconds}`;
  if (seconds == 0) seconds = '00';

  // get left-over number of minutes
  minutes = total_minutes - (total_hours * 60);
  if (minutes <= 9 && total_hours) minutes = `0${minutes}`;
  if (minutes == 0) minutes = '0';

  if (total_hours) string += `${total_hours}:`;
  if (minutes) string += `${minutes}:`;
  if (seconds) string += seconds;

  return string;
};

/**
 * Format time duration as a human-friendly sentence
 *
 * @param milliseconds int
 * @return string (eg 2+ hours)
 * */
const durationSentence = (milliseconds = null) => {
  if (milliseconds === null) return null;

  // get total values for each
  const totalSeconds = Math.floor(milliseconds / 1000);
  const totalMinutes = Math.floor(milliseconds / (1000 * 60));
  const totalHours = Math.floor(milliseconds / (1000 * 60 * 60));

  if (totalHours > 1) return `${totalHours}+ hrs`;
  if (totalMinutes > 1) return `${totalMinutes} mins`;
  if (totalSeconds) return `${totalSeconds} sec`;
  return '0 mins';
};

export default memo((props) => {
  if (props.data === undefined) {
    return null;
  }

  switch (props.type) {
    case 'total-time':
      var duration = 0;
      var tracks = props.data;
      for (let i = 0; i < tracks.length; i++) {
        if (tracks[i].duration) {
          duration += parseInt(tracks[i].duration);
        }
      }
      return durationSentence(duration);

    case 'length':
      return durationTime(props.data);

    case 'date':

      // A four-character date indicates just a year (rather than a full date)
      if (props.data.length == 4) {
        return props.data;

        // Digest as a date string
      }
      var date = new Date(props.data);
      return `${date.getDate()}/${date.getMonth() + 1}/${date.getFullYear()}`;


    case 'ago':
      var date = new Date(props.data);
      var diff = new Date() - date;
      var seconds = Math.floor(diff / 1000);
      var minutes = Math.floor(diff / (1000 * 60));
      var hours = Math.floor(diff / (1000 * 60 * 60));
      var days = Math.floor(diff / (1000 * 60 * 60 * 24));
      var weeks = Math.floor(diff / (1000 * 60 * 60 * 24 * 7));
      var years = Math.floor(diff / (1000 * 60 * 60 * 24 * 7 * 52));

      if (seconds < 60) {
        return `${seconds} second${seconds > 1 ? 's' : ''}`;
      } if (minutes < 60) {
        return `${minutes} minute${minutes > 1 ? 's' : ''}`;
      } if (hours < 24) {
        return `${hours} hour${hours > 1 ? 's' : ''}`;
      } if (days < 7) {
        return `${days} day${days > 1 ? 's' : ''}`;
      } if (weeks < 54) {
        return `${weeks} week${weeks > 1 ? 's' : ''}`;
      }
      return `${years} year${years > 1 ? 's' : ''}`;
    default:
      return null;
  }
});
