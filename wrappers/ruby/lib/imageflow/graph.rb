module Imageflow
  class Node
    attr_reader :index
    def initialize(graph:, index:)
      @g = graph
      @index = index
    end

    def delete!
      @g.context.call_method(:node_delete, @g.ptr_graph, @index)
      @index = -1
    end

    def add(*args)
      @g.add_child_node(@index, *args)
    end
  end
  class Graph

    def ptr_graph
      @pointer_reference.get_pointer(0)
    end
    def ptr_ptr_graph
      @pointer_reference
    end

    def initialize(context: , node_capacity: 10, edge_capacity: 10,
                   infobyte_capacity: 200, growth_factor: 2.0, ptr_graph: nil )
      @c = context
      @pointer_reference =  FFI::MemoryPointer.new(:size_t, 1, true)

      pointer = ptr_graph || @c.call_method(:graph_create, edge_capacity, node_capacity, infobyte_capacity, growth_factor)
      @pointer_reference.put_pointer(0, pointer)
    end



    def destroyed?
      @pointer_reference.nil? || @pointer_reference.get_pointer(0).null?
    end
    def destroy!
      @c.call_method(:graph_destroy, @pointer_reference.get_pointer(0))
      @pointer_reference = nil
    end

    def add_child_node(previous_node_index, node_type, *args)
      Node.new graph: self,
               index: @c.call_method("node_create_#{node_type}".to_sym, @pointer_reference, previous_node_index, *args)
    end

    def create_node(*args)
      add_child_node(-1, *args)
    end

    def deep_clone
      Graph.new context: @c, ptr_graph: @c.call_method(:graph_copy, ptr_graph)
    end

  end
end