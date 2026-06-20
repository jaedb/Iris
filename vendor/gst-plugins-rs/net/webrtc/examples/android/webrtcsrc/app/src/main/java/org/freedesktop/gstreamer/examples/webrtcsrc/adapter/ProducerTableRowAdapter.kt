package org.freedesktop.gstreamer.examples.webrtcsrc.adapter

import android.annotation.SuppressLint
import android.graphics.Color
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import androidx.recyclerview.widget.RecyclerView
import org.freedesktop.gstreamer.examples.webrtcsrc.R
import org.freedesktop.gstreamer.examples.webrtcsrc.model.Peer

class ProducerTableRowAdapter(private var producers: List<Peer>, private val onClick: (Peer) -> Unit) :
    RecyclerView.Adapter<ProducerTableRowAdapter.ViewHolder>() {

    override fun onCreateViewHolder(viewGroup: ViewGroup, i: Int): ViewHolder {
        val v: View = LayoutInflater.from(viewGroup.context)
            .inflate(R.layout.producer_table_row_layout, viewGroup, false)
        return ViewHolder(v, onClick)
    }

    override fun onBindViewHolder(viewHolder: ViewHolder, i: Int) {
        viewHolder.bind(producers[i])
    }

    override fun getItemCount() = producers.size

    class ViewHolder(itemView: View, val onClick: (Peer) -> Unit) : RecyclerView.ViewHolder(itemView) {
        private val tvProducerId: TextView = itemView.findViewById(R.id.tv_producer_id)
        private val tvProducerMeta: TextView = itemView.findViewById(R.id.tv_producer_meta)
        private var curProducer: Peer? = null

        init {
            itemView.setOnClickListener {
                curProducer?.let {
                    onClick(it)
                }
            }
            itemView.setOnFocusChangeListener { v, hasFocus ->
                if (hasFocus)
                    v.setBackgroundColor(Color.LTGRAY)
                else
                    v.setBackgroundColor(Color.TRANSPARENT)
            }
        }

        @SuppressLint("SetTextI18n")
        fun bind(producer: Peer) {
            tvProducerId.text = producer.id.substring(0..7) + "..."
            tvProducerMeta.text = producer.meta.toString()
            curProducer = producer
        }
    }

    @SuppressLint("NotifyDataSetChanged")
    fun setProducers(list: List<Peer>) {
        producers = list
        notifyDataSetChanged()
    }
}